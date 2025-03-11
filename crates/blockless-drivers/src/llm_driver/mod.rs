mod handle;
mod llamafile;
mod models;
mod provider;

use crate::LlmErrorKind;
use handle::HandleMap;
use llamafile::LlamafileProvider;
use models::SupportedModels;
use provider::{LLMProvider, Message, ProviderConfig};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

// Global variables (single instance of the context map)
static CONTEXTS: LazyLock<HandleMap<LlmContext<LlamafileProvider>>> =
    LazyLock::new(HandleMap::default);

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmOptions {
    pub system_message: String,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

#[derive(Clone)]
pub struct LlmContext<P: LLMProvider + Clone> {
    model: String,
    provider: P,
    options: LlmOptions,
    messages: Vec<Message>,
}

impl<P: LLMProvider + Clone> LlmContext<P> {
    async fn new(model: String, mut provider: P) -> Result<Self, LlmErrorKind> {
        provider
            .initialize(&ProviderConfig::default())
            .await
            .map_err(|_| LlmErrorKind::ModelInitializationFailed)?;

        Ok(Self {
            model,
            provider,
            options: LlmOptions::default(),
            messages: Vec::new(),
        })
    }

    fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
        });
    }
}

pub async fn llm_set_model(model: &str) -> Result<u32, LlmErrorKind> {
    // Parse model string to SupportedModels
    let supported_model: SupportedModels =
        model.parse().map_err(|_| LlmErrorKind::ModelNotSupported)?;

    // Create provider and context
    let provider = LlamafileProvider::new(supported_model);
    let context = LlmContext::new(model.to_string(), provider)
        .await
        .map_err(|_| LlmErrorKind::ModelInitializationFailed)?;

    tracing::info!("Model set: {}", model);

    Ok(CONTEXTS.insert(context))
}

pub async fn llm_get_model(handle: u32) -> Result<String, LlmErrorKind> {
    let ctx = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
    Ok(ctx.model.clone())
}

pub async fn llm_set_options(handle: u32, options: &[u8]) -> Result<(), LlmErrorKind> {
    let mut ctx = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
    let parsed_options: LlmOptions =
        serde_json::from_slice(options).map_err(|_| LlmErrorKind::ModelOptionsNotSet)?;

    if !parsed_options.system_message.is_empty() {
        ctx.messages.clear();
        ctx.add_message("system", &parsed_options.system_message);
    }

    ctx.options = parsed_options;
    Ok(())
}

pub async fn llm_get_options(handle: u32) -> Result<LlmOptions, LlmErrorKind> {
    let ctx = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
    Ok(ctx.options.clone())
}

pub async fn llm_prompt(handle: u32, prompt: &str) -> Result<(), LlmErrorKind> {
    let mut ctx = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
    ctx.add_message("user", prompt);
    Ok(())
}

pub async fn llm_read_response(handle: u32) -> Result<String, LlmErrorKind> {
    // Get all necessary data from the context before the async operation
    let (provider, messages) = {
        let ctx = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
        (ctx.provider.clone(), ctx.messages.clone())
    };

    // Perform the async chat operation
    let response = provider
        .chat(messages)
        .await
        .map_err(|_| LlmErrorKind::ModelCompletionFailed)?;

    // Update the context with the response
    let mut ctx = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
    ctx.add_message("assistant", &response.content.clone());

    Ok(response.content)
}

pub async fn llm_close(handle: u32) -> Result<(), LlmErrorKind> {
    if let Some(mut ctx) = CONTEXTS.remove(handle) {
        ctx.provider
            .shutdown()
            .map_err(|_| LlmErrorKind::ModelShutdownFailed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::{info, Level};
    use tracing_subscriber::FmtSubscriber;

    #[ignore]
    #[tokio::test]
    async fn test_llm_driver_e2e() {
        let _ = FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .with_test_writer()
            .try_init();

        info!("Starting E2E test for LLM driver");

        // 1. Set model and verify
        info!("Setting up model...");
        let handle = llm_set_model("Llama-3.2-1B-Instruct").await.unwrap();
        let model = llm_get_model(handle).await.unwrap();
        assert_eq!(model, "Llama-3.2-1B-Instruct");

        // 2. Set options and verify
        let system_message = r#"
        You are a helpful assistant.
        First time I ask, you name will be lucy.
        Second time I ask, you name will be bob.
        "#;
        let initial_options = LlmOptions {
            system_message: system_message.to_string(),
            temperature: Some(0.7),
            top_p: Some(0.9),
        };
        let options_bytes = serde_json::to_vec(&initial_options).unwrap();
        llm_set_options(handle, &options_bytes).await.unwrap();

        let retrieved_options = llm_get_options(handle).await.unwrap();
        assert_eq!(retrieved_options, initial_options);

        // First interaction
        let prompt1 = "What is your name?";
        llm_prompt(handle, prompt1).await.unwrap();
        let response1 = llm_read_response(handle).await.unwrap();
        info!("Q1: {}\nA1: {}", prompt1, response1);
        assert!(!response1.is_empty());

        // Second interaction
        let prompt2 = "What is your name?";
        llm_prompt(handle, prompt2).await.unwrap();
        let response2 = llm_read_response(handle).await.unwrap();
        info!("Q2: {}\nA2: {}", prompt2, response2);
        assert!(!response2.is_empty());

        // Update options
        let updated_options = LlmOptions {
            system_message: "You are now a mathematics tutor.".to_string(),
            temperature: Some(0.5),
            top_p: Some(0.95),
        };
        let updated_options_bytes = serde_json::to_vec(&updated_options).unwrap();
        llm_set_options(handle, &updated_options_bytes)
            .await
            .unwrap();

        let final_options = llm_get_options(handle).await.unwrap();
        assert_eq!(final_options, updated_options);

        // Clean up
        info!("Cleaning up...");
        llm_close(handle).await.unwrap();
    }
}
