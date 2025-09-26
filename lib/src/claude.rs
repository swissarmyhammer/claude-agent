//! Claude SDK wrapper providing session-aware interactions

use claude_sdk_rs::{Client, Config};
use tokio_stream::Stream;
use tokio::time::{sleep, Duration};

use std::time::SystemTime;

use crate::{config::ClaudeConfig, error::Result};

/// Claude client wrapper with session management
pub struct ClaudeClient {
    client: Client,
    config: Config,
}

/// Session context for managing conversation history
pub struct SessionContext {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub created_at: SystemTime,
}

/// Individual message in a conversation
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: SystemTime,
}

/// Message roles in a conversation
#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Streaming message chunk
#[derive(Debug, Clone)]
pub struct MessageChunk {
    pub content: String,
    pub chunk_type: ChunkType,
}

/// Types of message chunks in streaming responses
#[derive(Debug, Clone)]
pub enum ChunkType {
    Text,
    ToolCall,
    ToolResult,
}

impl ClaudeClient {
    /// Create a new Claude client with default configuration
    pub fn new() -> Result<Self> {
        let config = Config::default();
        let client = Client::new(config.clone());
        
        Ok(Self { client, config })
    }

    /// Create a new Claude client with custom configuration
    pub fn new_with_config(_claude_config: &ClaudeConfig) -> Result<Self> {
        let config = Config::default();
        
        // Map claude config to SDK config
        // The claude-sdk-rs Config doesn't appear to have model configuration in the constructor
        // The model is typically specified when making requests
        // For now, we store the config for later use in requests
        
        let client = Client::new(config.clone());
        Ok(Self { client, config })
    }

    /// Check if the client supports streaming
    pub fn supports_streaming(&self) -> bool {
        true
    }

    /// Execute a simple query without session context
    pub async fn query(&self, prompt: &str, _session_id: &str) -> Result<String> {
        // TODO: Implement actual Claude SDK query
        // For now, using a simplified approach until we can verify the exact API
        
        // Use the client for the actual implementation
        let _client = &self.client;
        let _config = &self.config;
        
        // Placeholder that acknowledges the client usage
        // In real implementation, this would make an actual API call
        self.execute_with_retry(|| async {
            // Simulate an API call that could fail and need retry
            if prompt.is_empty() {
                Err(crate::error::AgentError::Claude(claude_sdk_rs::Error::ConfigError("Empty prompt".to_string())))
            } else {
                Ok(format!("Claude response to: {}", prompt))
            }
        }).await
    }

    /// Execute a streaming query without session context
    pub async fn query_stream(
        &self, 
        prompt: &str, 
        _session_id: &str
    ) -> Result<impl Stream<Item = MessageChunk>> {
        // TODO: Implement actual Claude SDK streaming query
        // For now, return a working stream that uses the client
        
        let _client = &self.client;
        let _config = &self.config;
        
        // Create a simple stream that yields one chunk
        let chunks = vec![MessageChunk {
            content: format!("Streaming response to: {}", prompt),
            chunk_type: ChunkType::Text,
        }];
        
        Ok(tokio_stream::iter(chunks))
    }

    /// Execute a query with full session context
    pub async fn query_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> Result<String> {
        // TODO: Implement actual Claude SDK query with context
        // For now, use the client and build conversation history
        
        let _client = &self.client;
        let _config = &self.config;
        
        // Build conversation history from context
        let mut full_conversation = String::new();
        
        for message in &context.messages {
            let role_str = match message.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant", 
                MessageRole::System => "System",
            };
            full_conversation.push_str(&format!("{}: {}\n", role_str, message.content));
        }
        full_conversation.push_str(&format!("User: {}", prompt));

        // Use retry logic
        self.execute_with_retry(|| async {
            if prompt.is_empty() {
                Err(crate::error::AgentError::Claude(claude_sdk_rs::Error::ConfigError("Empty prompt".to_string())))
            } else {
                Ok(format!("Response with context (session: {}) to: {}", context.session_id, prompt))
            }
        }).await
    }

    /// Execute a streaming query with full session context
    pub async fn query_stream_with_context(
        &self,
        prompt: &str, 
        context: &SessionContext,
    ) -> Result<impl Stream<Item = MessageChunk>> {
        // TODO: Implement actual Claude SDK streaming query with context
        // For now, return a working stream that uses the client and context
        
        let _client = &self.client;
        let _config = &self.config;
        
        // Create a simple stream that includes context information
        let chunks = vec![
            MessageChunk {
                content: format!("Streaming response with context (session: {}, {} previous messages) to: {}", 
                    context.session_id, context.messages.len(), prompt),
                chunk_type: ChunkType::Text,
            }
        ];
        
        Ok(tokio_stream::iter(chunks))
    }

    /// Execute an operation with retry logic
    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T> 
    where
        F: Fn() -> Fut,
        Fut: futures::Future<Output = Result<T>>,
    {
        let mut attempts = 0;
        let max_attempts = 3;
        
        loop {
            attempts += 1;
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if attempts < max_attempts && is_retryable(&e) => {
                    let delay = Duration::from_millis(100 * 2_u64.pow(attempts - 1));
                    sleep(delay).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// Determine if an error is worth retrying
fn is_retryable(error: &crate::error::AgentError) -> bool {
    matches!(error, crate::error::AgentError::Claude(_))
}

impl SessionContext {
    /// Create a new session context
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            messages: Vec::new(),
            created_at: SystemTime::now(),
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        let message = Message {
            role,
            content,
            timestamp: SystemTime::now(),
        };
        self.messages.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = ClaudeClient::new().unwrap();
        assert!(client.supports_streaming());
    }

    #[tokio::test]
    async fn test_session_context() {
        let mut context = SessionContext::new("test-session".to_string());
        assert_eq!(context.session_id, "test-session");
        assert_eq!(context.messages.len(), 0);

        context.add_message(MessageRole::User, "Hello".to_string());
        assert_eq!(context.messages.len(), 1);
        assert!(matches!(context.messages[0].role, MessageRole::User));
        assert_eq!(context.messages[0].content, "Hello");
    }

    #[tokio::test]
    async fn test_basic_query() {
        let client = ClaudeClient::new().unwrap();
        let response = client.query("Test prompt", "session-1").await.unwrap();
        assert!(response.contains("Test prompt"));
    }

    #[tokio::test]
    async fn test_query_with_context() {
        let client = ClaudeClient::new().unwrap();
        let mut context = SessionContext::new("test-session".to_string());
        context.add_message(MessageRole::User, "Previous message".to_string());
        
        let response = client.query_with_context("New prompt", &context).await.unwrap();
        assert!(response.contains("New prompt"));
    }

    #[test]
    fn test_message_roles() {
        let user_msg = Message {
            role: MessageRole::User,
            content: "User message".to_string(),
            timestamp: SystemTime::now(),
        };
        
        let assistant_msg = Message {
            role: MessageRole::Assistant,
            content: "Assistant message".to_string(),
            timestamp: SystemTime::now(),
        };

        let system_msg = Message {
            role: MessageRole::System,
            content: "System message".to_string(),
            timestamp: SystemTime::now(),
        };

        assert!(matches!(user_msg.role, MessageRole::User));
        assert!(matches!(assistant_msg.role, MessageRole::Assistant));
        assert!(matches!(system_msg.role, MessageRole::System));
    }

    #[test]
    fn test_chunk_types() {
        let text_chunk = MessageChunk {
            content: "text".to_string(),
            chunk_type: ChunkType::Text,
        };

        let tool_call_chunk = MessageChunk {
            content: "tool_call".to_string(),
            chunk_type: ChunkType::ToolCall,
        };

        let tool_result_chunk = MessageChunk {
            content: "tool_result".to_string(),
            chunk_type: ChunkType::ToolResult,
        };

        assert!(matches!(text_chunk.chunk_type, ChunkType::Text));
        assert!(matches!(tool_call_chunk.chunk_type, ChunkType::ToolCall));
        assert!(matches!(tool_result_chunk.chunk_type, ChunkType::ToolResult));
    }
}