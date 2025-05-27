mod handle;
mod llamafile;
mod mcp;
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
    pub tools_sse_urls: Option<Vec<String>>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

#[derive(Clone)]
pub struct LlmContext<P: LLMProvider> {
    model: String,
    provider: Arc<P>,
    options: LlmOptions,
    messages: Arc<Mutex<Vec<Message>>>,
    tools_map: Option<Arc<mcp::ToolsMap>>,
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
            tools_map: None,
        })
    }

    fn add_message(&mut self, role: Role, content: String) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(Message { role, content });
    }

    /// Get a reference to the tools map
    pub fn get_tools_map(&self) -> Option<Arc<mcp::ToolsMap>> {
        self.tools_map.clone()
    }

    /// Set the tools map
    pub fn set_tools_map(&mut self, tools_map: mcp::ToolsMap) {
        self.tools_map = Some(Arc::new(tools_map));
    }
}

pub async fn llm_set_model<F>(model: &str, url_permission_checker: F) -> Result<u32, LlmErrorKind>
where
    F: Fn(&url::Url) -> bool,
{
    // Parse model string to Models
    let supported_model: Models = model.parse().map_err(|_| LlmErrorKind::ModelNotSupported)?;

    // Check URL permissions for Models::Url variant
    if let Models::Url(ref url) = supported_model {
        if !url_permission_checker(url) {
            tracing::error!("Permission denied for model URL: {}", url);
            return Err(LlmErrorKind::PermissionDeny);
        }
    }

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

    // Construct system prompt with tools map
    let (system_prompt, tools_map) = mcp::construct_system_prompt_with_tools(&parsed_options).await;

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

            // Set tools map if present
            if let Some(tools_map) = tools_map {
                ctx.set_tools_map(tools_map);
            }

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
    let (provider, messages, tools_map) = {
        let ctx_arc = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
        let ctx = ctx_arc.lock().unwrap();
        (
            ctx.provider.clone(),
            ctx.messages.lock().unwrap().clone(),
            ctx.get_tools_map(),
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

    // If no tools map, just return the response
    let Some(tools_map) = tools_map else {
        return Ok(response.content);
    };

    // Ensure tools map has at least one accessible tool
    let accessible_tools = tools_map
        .iter()
        .filter(|(_, tool)| tool.is_accessible)
        .collect::<Vec<_>>();
    if accessible_tools.is_empty() {
        return Ok(response.content);
    }

    tracing::debug!(
        "Attempting to process LLM response with tools: {}",
        response.content
    );

    // Process any function call in the response
    match mcp::process_function_call(&response.content, &tools_map).await {
        // No function call, just return the response
        mcp::ProcessFunctionResult::NoFunctionCall => {
            tracing::debug!("No function call detected in the response");
            Ok(response.content)
        }

        // Function call executed with result
        mcp::ProcessFunctionResult::FunctionExecuted(result) => {
            tracing::debug!("Function call executed with result: {}", result);

            // Add the tool response to the context
            CONTEXTS
                .with_instance_mut(handle, |ctx| {
                    ctx.add_message(Role::Tool, result.clone());
                })
                .ok_or(LlmErrorKind::ModelNotSet)?;

            // Get updated messages for final response - only get them once
            let updated_messages = {
                let ctx_arc = CONTEXTS.get(handle).ok_or(LlmErrorKind::ModelNotSet)?;
                let ctx = ctx_arc.lock().unwrap();
                ctx.messages.lock().unwrap().clone()
            };

            // Get final response after tool call
            let llm_response = provider.chat(&updated_messages).await.map_err(|err| {
                tracing::error!("Model completion failed: {:?}", err);
                LlmErrorKind::ModelCompletionFailed
            })?;

            // Add the final assistant message to the context
            CONTEXTS
                .with_instance_mut(handle, |ctx| {
                    ctx.add_message(Role::Assistant, llm_response.content.clone());
                })
                .ok_or(LlmErrorKind::ModelNotSet)?;

            Ok(llm_response.content)
        }

        mcp::ProcessFunctionResult::Error(error_message) => {
            tracing::error!("MCP function call error: {}", error_message);
            Err(LlmErrorKind::MCPFunctionCallError)
        }
    }
}

pub async fn llm_close(handle: u32) -> Result<(), LlmErrorKind> {
    if let Some(ctx) = CONTEXTS.remove(handle) {
        // Try to unwrap the Arc to get exclusive ownership
        let provider = match Arc::try_unwrap(ctx.provider) {
            Ok(provider) => provider,
            Err(arc_provider) => {
                // If we can't get exclusive ownership, log and force a clone to shutdown
                tracing::error!(
                    "Provider has multiple references during shutdown, forcing shutdown"
                );
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

    /// Helper function to validate URL permissions without downloading the model
    fn validate_model_url_permission<F>(
        model: &str,
        url_permission_checker: F,
    ) -> Result<(), LlmErrorKind>
    where
        F: Fn(&url::Url) -> bool,
    {
        let supported_model: Models = model.parse().map_err(|_| LlmErrorKind::ModelNotSupported)?;
        if let Models::Url(ref url) = supported_model {
            if !url_permission_checker(url) {
                tracing::error!("Permission denied for model URL: {}", url);
                return Err(LlmErrorKind::PermissionDeny);
            }
        }
        Ok(())
    }

    #[ignore = "requires downloading large LLM model"]
    #[tokio::test]
    async fn test_llm_driver_e2e() {
        let _ = FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        // Set model and verify
        tracing::info!("Setting up model...");
        let handle = llm_set_model("Llama-3.2-1B-Instruct", |_| true)
            .await
            .unwrap();
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
            tools_sse_urls: None,
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
            tools_sse_urls: None,
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

    #[tokio::test]
    #[ignore = "requires running MCP server and downloading large LLM model"]
    async fn test_llm_driver_custom_model_mcp() {
        let _ = FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        // Set model and verify
        tracing::info!("Setting up model...");
        let handle = llm_set_model("https://huggingface.co/Mozilla/Meta-Llama-3.1-8B-Instruct-llamafile/resolve/main/Meta-Llama-3.1-8B-Instruct.Q6_K.llamafile", |_| true).await.unwrap();
        let model = llm_get_model(handle).await.unwrap();
        assert_eq!(
            model,
            "https://huggingface.co/Mozilla/Meta-Llama-3.1-8B-Instruct-llamafile/resolve/main/Meta-Llama-3.1-8B-Instruct.Q6_K.llamafile"
        );

        // Set options and verify with MCP tools
        let initial_options = LlmOptions {
            tools_sse_urls: Some(vec!["http://localhost:3001/sse".into()]),
            ..Default::default()
        };
        let options_bytes = serde_json::to_vec(&initial_options).unwrap();
        llm_set_options(handle, &options_bytes).await.unwrap();

        let retrieved_options = llm_get_options(handle).await.unwrap();
        assert_eq!(retrieved_options, initial_options);

        // Try to use MCP server to add numbers
        let prompt = "Add the following numbers: 1215, 2213";
        llm_prompt(handle, prompt).await.unwrap();
        let response = llm_read_response(handle).await.unwrap();
        tracing::info!("\nQ: {}\nA: {}", prompt, response);
        assert!(!response.is_empty());

        // Clean up
        tracing::debug!("Cleaning up...");
        llm_close(handle).await.unwrap();
    }

    #[tokio::test]
    async fn test_llm_url_permission_check() {
        let _ = FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .try_init();

        // Test with permission checker that allows all URLs
        let allow_all_checker = |_: &url::Url| true;
        let result = validate_model_url_permission(
            "https://huggingface.co/Mozilla/Llama-3.2-1B-Instruct-llamafile/resolve/main/Llama-3.2-1B-Instruct.Q6_K.llamafile",
            allow_all_checker,
        );
        assert!(result.is_ok());

        // Test with permission checker that denies all URLs
        let deny_all_checker = |_: &url::Url| false;
        let result = validate_model_url_permission(
            "https://huggingface.co/Mozilla/Llama-3.2-1B-Instruct-llamafile/resolve/main/Llama-3.2-1B-Instruct.Q6_K.llamafile",
            deny_all_checker,
        );
        assert!(matches!(result, Err(LlmErrorKind::PermissionDeny)));

        // Test with known model (should not trigger permission check)
        let deny_all_checker = |_: &url::Url| false;
        let result = validate_model_url_permission("Llama-3.2-1B-Instruct", deny_all_checker);
        assert!(result.is_ok());

        // Test with invalid model string
        let allow_all_checker = |_: &url::Url| true;
        let result = validate_model_url_permission("InvalidModel", allow_all_checker);
        assert!(matches!(result, Err(LlmErrorKind::ModelNotSupported)));

        // Test with URL that fails built-in security validation (should return ModelNotSupported, not PermissionDeny)
        let allow_all_checker = |_: &url::Url| true;
        let result = validate_model_url_permission(
            "https://malicious.com/bad-model.llamafile",
            allow_all_checker,
        );
        assert!(matches!(result, Err(LlmErrorKind::ModelNotSupported)));

        // Test with URL that fails built-in security validation due to HTTP (not HTTPS)
        let result = validate_model_url_permission(
            "http://huggingface.co/model.llamafile",
            allow_all_checker,
        );
        assert!(matches!(result, Err(LlmErrorKind::ModelNotSupported)));

        // Test with selective permission checker on valid URL
        let selective_checker = |url: &url::Url| {
            // Allow only URLs that contain "Llama" in the path
            url.path().contains("Llama")
        };

        // This should pass both security validation and permission check
        let result = validate_model_url_permission(
            "https://huggingface.co/Mozilla/Llama-3.2-1B-Instruct-llamafile/resolve/main/Llama-3.2-1B-Instruct.Q6_K.llamafile",
            selective_checker,
        );
        assert!(result.is_ok());

        // This should pass security validation but fail permission check
        let result = validate_model_url_permission(
            "https://huggingface.co/Mozilla/Mistral-7B-Instruct-llamafile/resolve/main/Mistral-7B-Instruct.Q6_K.llamafile",
            selective_checker,
        );
        assert!(matches!(result, Err(LlmErrorKind::PermissionDeny)));
    }
}
