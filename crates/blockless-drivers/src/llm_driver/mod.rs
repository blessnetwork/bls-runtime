mod handle;
mod llamafile;
mod models;
mod provider;

use crate::{LlmErrorKind, llm_driver::provider::Role};
use handle::HandleMap;
use llamafile::LlamafileProvider;
use models::Models;
use provider::{LLMProvider, Message, ProviderConfig};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock, Mutex};

// Global variables (single instance of the context map)
static CONTEXTS: LazyLock<HandleMap<LlmContext<LlamafileProvider>>> =
    LazyLock::new(HandleMap::default);

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmOptions {
    pub system_message: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

#[derive(Clone)]
pub struct LlmContext<P: LLMProvider> {
    model: String,
    provider: Arc<P>,
    options: LlmOptions,
    messages: Arc<Mutex<Vec<Message>>>,
}

impl<P: LLMProvider + Clone> LlmContext<P> {
    async fn new(model: String, mut provider: P) -> Result<Self, LlmErrorKind> {
        provider
            .initialize(ProviderConfig::default())
            .await
            .map_err(|_| LlmErrorKind::ModelInitializationFailed)?;

        Ok(Self {
            model,
            provider: Arc::new(provider),
            options: LlmOptions::default(),
            messages: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn add_message(&mut self, role: Role, content: String) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(Message { role, content });
    }
}

pub async fn llm_set_model(model: &str) -> Result<u32, LlmErrorKind> {
    // Parse model string to Models
    let supported_model: Models = model.parse().map_err(|_| LlmErrorKind::ModelNotSupported)?;

    // Create provider and context
    let provider = LlamafileProvider::new(supported_model);
    let context = LlmContext::new(model.to_string(), provider)
        .await
        .map_err(|_| LlmErrorKind::ModelInitializationFailed)?;

    tracing::info!("Model set: {}", model);

    Ok(CONTEXTS.insert(context))
}

pub async fn llm_get_model(handle: u32) -> Result<String, LlmErrorKind> {
    CONTEXTS
        .with_instance(handle, |ctx| ctx.model.clone())
        .ok_or(LlmErrorKind::ModelNotSet)
}

pub async fn llm_set_options(handle: u32, options: &[u8]) -> Result<(), LlmErrorKind> {
    // Parse options first
    let parsed_options: LlmOptions = serde_json::from_slice(options).map_err(|err| {
        tracing::error!("Failed to parse options: {:?}", err);
        LlmErrorKind::ModelOptionsNotSet
    })?;

    let system_prompt = parsed_options.system_message.clone().unwrap_or("You are a helpful assistant.".to_string());

    // Now update the context after the async work
    CONTEXTS
        .with_instance_mut(handle, move |ctx| {
            // Clear messages and add new system prompt
            let mut messages = ctx.messages.lock().unwrap();
            messages.clear();

            // Add system message and set tools
            messages.push(Message {
                role: Role::System,
                content: system_prompt,
            });

            // Drop the messages guard
            drop(messages);

            // Sync options - required by SDK for verification
            ctx.options = parsed_options;
        })
        .ok_or(LlmErrorKind::ModelNotSet)?;

    Ok(())
}

pub async fn llm_get_options(handle: u32) -> Result<LlmOptions, LlmErrorKind> {
    CONTEXTS
        .with_instance(handle, |ctx| ctx.options.clone())
        .ok_or(LlmErrorKind::ModelNotSet)
}

pub async fn llm_prompt(handle: u32, prompt: &str) -> Result<(), LlmErrorKind> {
    CONTEXTS
        .with_instance_mut(handle, |ctx| {
            ctx.add_message(Role::User, prompt.to_string());
        })
        .ok_or(LlmErrorKind::ModelNotSet)?;
    Ok(())
}

pub async fn llm_read_response(handle: u32) -> Result<String, LlmErrorKind> {
    // Use a block to ensure the lock is dropped before any async calls
    // MutexGuard dropped after the block
    let (provider, messages) = {
        let ctx_arc = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
        let ctx = ctx_arc.lock().unwrap();
        (
            ctx.provider.clone(),
            ctx.messages.lock().unwrap().clone(),
        )
    };

    // Perform the async chat operation with the snapshot of data
    let response = provider.chat(&messages).await.map_err(|err| {
        tracing::error!("Model completion failed: {:?}", err);
        LlmErrorKind::ModelCompletionFailed
    })?;

    // Add the assistant message to the context
    CONTEXTS
        .with_instance_mut(handle, |ctx| {
            ctx.add_message(Role::Assistant, response.content.clone());
        })
        .ok_or(LlmErrorKind::ModelNotSet)?;

    Ok(response.content)
}

pub async fn llm_close(handle: u32) -> Result<(), LlmErrorKind> {
    if let Some(ctx) = CONTEXTS.remove(handle) {
        // Try to unwrap the Arc to get exclusive ownership
        let provider = match Arc::try_unwrap(ctx.provider) {
            Ok(provider) => provider,
            Err(arc_provider) => {
                // If we can't get exclusive ownership, log and force a clone to shutdown
                tracing::error!("Provider has multiple references during shutdown, forcing shutdown");
                let mut provider_clone = (*arc_provider).clone();
                provider_clone
                    .shutdown()
                    .map_err(|_| LlmErrorKind::ModelShutdownFailed)?;
                return Ok(());
            }
        };

        // exclusive ownership ensured, shutdown properly
        let mut provider = provider;
        provider
            .shutdown()
            .map_err(|_| LlmErrorKind::ModelShutdownFailed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::FmtSubscriber;

    #[ignore = "requires downloading large LLM model"]
    #[tokio::test]
    async fn test_llm_driver_e2e() {
        let _ = FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        // Set model and verify
        tracing::info!("Setting up model...");
        let handle = llm_set_model("Llama-3.2-1B-Instruct").await.unwrap();
        let model = llm_get_model(handle).await.unwrap();
        assert_eq!(model, "Llama-3.2-1B-Instruct");

        // Set options and verify
        let system_message = r#"
        You are a helpful assistant.
        First time I ask, you name will be lucy.
        Second time I ask, you name will be bob.
        "#;
        let initial_options = LlmOptions {
            system_message: Some(system_message.to_string()),
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
        tracing::info!("Q1: {}\nA1: {}", prompt1, response1);
        assert!(!response1.is_empty());

        // Second interaction
        let prompt2 = "What is your name?";
        llm_prompt(handle, prompt2).await.unwrap();
        let response2 = llm_read_response(handle).await.unwrap();
        tracing::info!("Q2: {}\nA2: {}", prompt2, response2);
        assert!(!response2.is_empty());

        // Update options
        let updated_options = LlmOptions {
            system_message: Some("You are now a mathematics tutor.".to_string()),
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
        tracing::info!("Cleaning up...");
        llm_close(handle).await.unwrap();
    }
}
