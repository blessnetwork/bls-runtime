use crate::llm_driver::{
    models::SupportedModels,
    provider::{LLMProvider, Message, ProviderConfig, ProviderError},
};
use reqwest;
use std::{
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
};
use tokio::fs;
use tracing::{debug, info};

/// The base path for the models from home directory
const BASE_MODEL_PATH: &str = ".blessnet/models";
const LLAMAFILE_BASE_URL: &str = "https://huggingface.co/Mozilla";

#[derive(Debug)]
pub struct LlamafileProvider {
    pub model: SupportedModels,
    process: Option<Child>,
    config: ProviderConfig,
}

impl Default for LlamafileProvider {
    fn default() -> Self {
        Self::new(SupportedModels::Llama323BInstruct(None))
    }
}

impl Clone for LlamafileProvider {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            process: None,
            config: self.config.clone(),
        }
    }
}

impl LlamafileProvider {
    pub fn new(model: SupportedModels) -> Self {
        Self {
            model,
            process: None,
            config: ProviderConfig::default(),
        }
    }

    fn model_file_url(&self) -> url::Url {
        let model_name = self.model.model_repo();
        let model_file = self.model.model_file();
        let url = format!(
            "{}/{}/resolve/main/{}?download=true",
            LLAMAFILE_BASE_URL, model_name, model_file
        );
        url::Url::parse(&url).unwrap()
    }

    fn get_model_path(&self) -> PathBuf {
        std::env::var_os("HOME")
            .map(|home| {
                PathBuf::from(home)
                    .join(BASE_MODEL_PATH)
                    .join(self.model.model_file())
            })
            .unwrap()
    }

    async fn ensure_model_exists(&self) -> Result<(), ProviderError> {
        let model_path = self.get_model_path();
        if !model_path.exists() {
            info!(
                "Model not found, downloading to `{}`...",
                model_path.display()
            );
            self.download_model().await?;
        }
        Ok(())
    }

    async fn download_model(&self) -> Result<(), ProviderError> {
        let model_path = self.get_model_path();
        let model_dir = model_path.parent().unwrap();
        fs::create_dir_all(model_dir)
            .await
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;

        let url = self.model_file_url();
        let response = reqwest::get(url)
            .await
            .map_err(|e| ProviderError::CommunicationError(e.to_string()))?;
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::CommunicationError(e.to_string()))?;

        let mut file = std::fs::File::create(&model_path)
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;
        file.write_all(&bytes)
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file
                .metadata()
                .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?
                .permissions();
            perms.set_mode(0o755);
            file.set_permissions(perms)
                .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;
        }

        Ok(())
    }

    fn start_server(&mut self) -> Result<(), ProviderError> {
        let model_path = self.get_model_path();
        let process = Command::new(&model_path)
            .arg("--server")
            .arg("--nobrowser")
            .arg("--host")
            .arg(&self.config.host)
            .arg("--port")
            .arg(self.config.port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;

        self.process = Some(process);
        debug!(
            "Started llamafile server on {}:{}",
            self.config.host, self.config.port
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl LLMProvider for LlamafileProvider {
    async fn initialize(&mut self, config: &ProviderConfig) -> Result<(), ProviderError> {
        info!(
            "Initializing Llamafile provider for model: {}",
            self.model.to_string()
        );
        self.config = config.clone();
        self.ensure_model_exists().await?;
        self.start_server()?;

        // Wait for server to start
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        Ok(())
    }

    async fn chat(&self, messages: Vec<Message>) -> Result<Message, ProviderError> {
        let client = reqwest::Client::new();
        let url = format!(
            "http://{}:{}/v1/chat/completions",
            self.config.host, self.config.port
        );

        let payload = serde_json::json!({
          "model": "LLaMA_CPP",
          "messages": messages,
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(self.config.timeout)
            .send()
            .await
            .map_err(|e| ProviderError::CommunicationError(e.to_string()))?;

        let response_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        let content = response_data["choices"][0]["message"].clone();
        serde_json::from_value(content).map_err(|e| ProviderError::InvalidResponse(e.to_string()))
    }

    fn shutdown(&mut self) -> Result<(), ProviderError> {
        if let Some(mut process) = self.process.take() {
            process
                .kill()
                .map_err(|e| ProviderError::ShutdownError(e.to_string()))?;
            process
                .wait()
                .map_err(|e| ProviderError::ShutdownError(e.to_string()))?;
            debug!("Stopped llamafile server");
        }
        Ok(())
    }
}

impl Drop for LlamafileProvider {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            debug!("Failed to shutdown llamafile server: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    fn init_test_logging() {
        let _ = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .with_test_writer()
            .try_init();
    }

    #[ignore]
    #[tokio::test]
    async fn test_llamafile_lifecycle() {
        init_test_logging();

        let mut provider = LlamafileProvider::new(SupportedModels::Llama321BInstruct(None));
        provider
            .initialize(&ProviderConfig::default())
            .await
            .unwrap();

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are a helpful assistant.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "Hello!".to_string(),
            },
        ];

        let response = provider.chat(messages).await.unwrap();
        info!("Chat response: {:?}", response);
        assert!(!response.content.is_empty());
    }

    #[test]
    fn test_model_parsing() {
        assert!(SupportedModels::from_str("Llama-3.2-1B-Instruct").is_ok());
        assert!(SupportedModels::from_str("Llama-3.2-1B-Instruct-Q6_K").is_ok());

        assert!(SupportedModels::from_str("Llama-3.2-3B-Instruct").is_ok());
        assert!(SupportedModels::from_str("Llama-3.2-3B-Instruct-Q6_K").is_ok());
        assert!(SupportedModels::from_str("unsupported-model").is_err());
    }
}
