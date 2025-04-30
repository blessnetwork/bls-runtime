use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
        }
    }
}

#[derive(Debug)]
pub enum ProviderError {
    InitializationFailed(String),
    CommunicationError(String),
    InvalidResponse(String),
    ShutdownError(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitializationFailed(msg) => write!(f, "Initialization failed: {}", msg),
            Self::CommunicationError(msg) => write!(f, "Communication error: {}", msg),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            Self::ShutdownError(msg) => write!(f, "Shutdown error: {}", msg),
        }
    }
}

impl std::error::Error for ProviderError {}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub host: String,
    pub port: u16,
    pub timeout: std::time::Duration,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            timeout: std::time::Duration::from_secs(30),
        }
    }
}

#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync + std::fmt::Debug {
    /// Initialize the provider with any necessary setup
    async fn initialize(&mut self, config: &ProviderConfig) -> Result<(), ProviderError>;

    /// Generate a chat completion based on the conversation history
    async fn chat(&self, messages: Vec<Message>) -> Result<Message, ProviderError>;

    /// Perform any necessary cleanup when shutting down
    fn shutdown(&mut self) -> Result<(), ProviderError>;
}
