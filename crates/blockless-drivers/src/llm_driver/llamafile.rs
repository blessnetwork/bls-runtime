use crate::llm_driver::{
    models::Models,
    provider::{LLMProvider, Message, ProviderConfig, ProviderError},
};
use reqwest;
use std::{
    io::ErrorKind,
    path::PathBuf,
    process::{Child, Command, Stdio},
};
use tokio::fs;
use tracing::{debug, info};

/// The base path for the models from home directory
const BASE_MODEL_PATH: &str = ".blessnet/models";
const LLAMAFILE_BASE_HUGGINGFACE_URL: &str = "https://huggingface.co";

#[derive(Debug)]
pub struct LlamafileProvider {
    pub model: Models,
    process: Option<Child>,
    config: ProviderConfig,
}

impl Default for LlamafileProvider {
    fn default() -> Self {
        Self::new(Models::Llama323BInstruct(None))
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
    pub fn new(model: Models) -> Self {
        Self {
            model,
            process: None,
            config: ProviderConfig::default(),
        }
    }

    fn model_file_url(&self) -> url::Url {
        match self.model.model_repo() {
            Some(model_repo) => {
                let model_file = self.model.model_file();
                let url = format!(
                    "{}/{}/resolve/main/{}?download=true",
                    LLAMAFILE_BASE_HUGGINGFACE_URL, model_repo, model_file
                );
                url::Url::parse(&url).unwrap()
            }
            None => {
                // The model file must be a valid URL at this point
                let model_file_url = self.model.to_string();
                url::Url::parse(&model_file_url).unwrap()
            }
        }
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

    fn start_server(&mut self) -> Result<(), ProviderError> {
        let model_path = self.get_model_path();

        let command_str = format!(
            "{} --server --nobrowser --host {} --port {}",
            model_path.display(),
            self.config.host,
            self.config.port
        );
        debug!("Starting llamafile server with command: `{}`", command_str);

        // Build the command
        let mut command = Command::new(&model_path);
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Spawn the process
        let process = command.spawn().map_err(|e| match e.kind() {
            ErrorKind::NotFound => {
                ProviderError::LLamaFileServerError("LlamaFile not found".to_string())
            }
            ErrorKind::PermissionDenied => ProviderError::LLamaFileServerError(
                "Permission denied; please re-download the model".to_string(),
            ),
            _ => ProviderError::LLamaFileServerError(e.to_string()),
        })?;

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

/// Downloads a model file with resumable download support.
///
/// # Features
/// - Resumes interrupted downloads using HTTP Range headers
/// - Uses .part files for tracking partial downloads
/// - Shows download progress and verifies file size
/// - Sets executable permissions on completion
///
/// # Arguments
/// * `url` - Source URL for the model
/// * `model_path` - Destination path to save the model
///
/// # Errors
/// Returns ProviderError for directory creation failures, network errors,
/// server errors (404, etc.), or file operation failures.
async fn download_model(url: url::Url, model_path: &PathBuf) -> Result<(), ProviderError> {
    // create the model directory if it doesn't exist
    if let Some(model_dir) = model_path.parent() {
        fs::create_dir_all(model_dir).await.map_err(|e| {
            ProviderError::InitializationFailed(format!("Failed to create model directory: {}", e))
        })?;
    } else {
        return Err(ProviderError::InitializationFailed(
            "Invalid model path: no parent directory".to_string(),
        ));
    }

    // perform a HEAD request to check if the model file exists
    let client = reqwest::Client::new();
    let head_response = client
        .head(url.clone())
        .send()
        .await
        .map_err(|e| ProviderError::CommunicationError(e.to_string()))?;
    let status_code = head_response.status().as_u16();
    if status_code >= 400 {
        return Err(ProviderError::ServerResponseError(format!(
            "Failed to download model; status code: {}",
            status_code
        )));
    }

    // Get total size from headers (prefer x-linked-size if available)
    let total_size = head_response
        .headers()
        .get("x-linked-size")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .or_else(|| {
            head_response
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
        })
        .unwrap_or(0);
    if total_size == 0 {
        info!("Warning: Unable to determine total file size for download");
    } else {
        info!("Total download size: {} bytes", total_size);
    }

    // Use a .part file for partial downloads
    let part_path = model_path.with_extension("part");

    // Check if partial file exists and get its size
    let file_size = if part_path.exists() {
        fs::metadata(&part_path).await.map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    // If the file is already complete, just rename it
    if total_size > 0 && file_size == total_size {
        info!("Download already complete, finalizing...");
        fs::rename(&part_path, model_path)
            .await
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;
    } else {
        // File is incomplete or size unknown, start/resume download
        let mut file = if file_size > 0 {
            info!(
                "Resuming download from byte {} of {} ({}%)",
                file_size,
                total_size,
                if total_size > 0 {
                    file_size * 100 / total_size
                } else {
                    0
                }
            );
            tokio::fs::OpenOptions::new()
                .write(true)
                .append(true)
                .open(&part_path)
                .await
                .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?
        } else {
            info!("Starting new download to `{}`...", part_path.display());
            tokio::fs::File::create(&part_path)
                .await
                .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?
        };

        // Create request with Range header if resuming
        let mut req = client.get(url.clone());
        if file_size > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={}-", file_size));
        }

        let mut response = req
            .send()
            .await
            .map_err(|e| ProviderError::ServerResponseError(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ProviderError::ServerResponseError(format!(
                "Download failed; status code: {}",
                response.status()
            )));
        }

        // Stream response to file, log progress periodically
        let mut downloaded = file_size;
        let mut last_percentage = downloaded * 100 / total_size.max(1);
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| ProviderError::StreamError(e.to_string()))?
        {
            tokio::io::copy_buf(&mut chunk.as_ref(), &mut file)
                .await
                .map_err(|e| ProviderError::StreamError(e.to_string()))?;

            downloaded += chunk.len() as u64;

            // Log progress periodically
            if total_size > 0 {
                let percentage = downloaded * 100 / total_size;
                if percentage > last_percentage && percentage % 10 == 0 {
                    info!("Download progress: {}%", percentage);
                    last_percentage = percentage;
                }
            }
        }

        // Sync file to ensure data is written to disk
        file.sync_all()
            .await
            .map_err(|e| ProviderError::StreamError(e.to_string()))?;

        // Verify file size
        if total_size > 0 {
            let final_size = fs::metadata(&part_path).await.map(|m| m.len()).unwrap_or(0);
            if final_size != total_size {
                info!(
                    "Warning: Downloaded file size ({}) doesn't match expected size ({})",
                    final_size, total_size
                );
                // Continue anyway as the size might be inaccurate
            }
        }

        // Rename part file to final file to complete the download
        fs::rename(&part_path, model_path)
            .await
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;

        info!("Download completed successfully");
    }

    // Set executable permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(model_path)
            .await
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(model_path, perms)
            .await
            .map_err(|e| ProviderError::InitializationFailed(e.to_string()))?;
    }

    Ok(())
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
