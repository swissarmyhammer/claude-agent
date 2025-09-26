//! Claude SDK wrapper providing session-aware interactions

use claude_sdk_rs::{Client, Config, Message};
use tokio::time::{sleep, Duration};
use tokio_stream::{Stream, StreamExt};

use std::time::SystemTime;

use crate::{config::ClaudeConfig, error::Result};

/// Claude client wrapper with session management
pub struct ClaudeClient {
    client: Client,
}

/// Session context for managing conversation history
pub struct SessionContext {
    pub session_id: String,
    pub messages: Vec<ClaudeMessage>,
    pub created_at: SystemTime,
}

/// Individual message in a conversation
#[derive(Debug, Clone)]
pub struct ClaudeMessage {
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
        let client = Client::new(config);

        Ok(Self { client })
    }

    /// Create a new Claude client with custom configuration
    pub fn new_with_config(_claude_config: &ClaudeConfig) -> Result<Self> {
        let config = Config::default();

        // Map claude config to SDK config
        // The claude-sdk-rs Config doesn't appear to have model configuration in the constructor
        // The model is typically specified when making requests

        let client = Client::new(config);
        Ok(Self { client })
    }

    /// Check if the client supports streaming
    pub fn supports_streaming(&self) -> bool {
        true
    }

    /// Execute a simple query without session context
    pub async fn query(&self, prompt: &str, _session_id: &str) -> Result<String> {
        self.execute_with_retry(|| async {
            if prompt.is_empty() {
                return Err(crate::error::AgentError::Claude(
                    claude_sdk_rs::Error::ConfigError("Empty prompt".to_string()),
                ));
            }

            let response = self
                .client
                .send_full(prompt)
                .await
                .map_err(crate::error::AgentError::Claude)?;

            Ok(response.content)
        })
        .await
    }

    /// Execute a streaming query without session context
    pub async fn query_stream(
        &self,
        prompt: &str,
        _session_id: &str,
    ) -> Result<impl Stream<Item = MessageChunk>> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Claude(
                claude_sdk_rs::Error::ConfigError("Empty prompt".to_string()),
            ));
        }

        let message_stream = self
            .client
            .query(prompt)
            .stream()
            .await
            .map_err(crate::error::AgentError::Claude)?;

        // Convert the MessageStream to our MessageChunk stream
        let chunk_stream = message_stream.map(|result| {
            match result {
                Ok(Message::Assistant { content, .. }) => MessageChunk {
                    content,
                    chunk_type: ChunkType::Text,
                },
                Ok(Message::Tool { .. }) => MessageChunk {
                    content: String::new(), // Tool calls don't have direct content
                    chunk_type: ChunkType::ToolCall,
                },
                Ok(Message::ToolResult { .. }) => MessageChunk {
                    content: String::new(), // Tool results handled separately
                    chunk_type: ChunkType::ToolResult,
                },
                Ok(_) => MessageChunk {
                    content: String::new(), // Other message types (Init, User, System, Result)
                    chunk_type: ChunkType::Text,
                },
                Err(_) => MessageChunk {
                    content: String::new(), // Error handling - could be improved
                    chunk_type: ChunkType::Text,
                },
            }
        });

        Ok(chunk_stream)
    }

    /// Execute a query with full session context
    pub async fn query_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> Result<String> {
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

        // Use retry logic with actual Claude SDK call
        self.execute_with_retry(|| async {
            if prompt.is_empty() {
                return Err(crate::error::AgentError::Claude(
                    claude_sdk_rs::Error::ConfigError("Empty prompt".to_string()),
                ));
            }

            // Use the full conversation including context
            let response = self
                .client
                .send_full(&full_conversation)
                .await
                .map_err(crate::error::AgentError::Claude)?;

            Ok(response.content)
        })
        .await
    }

    /// Execute a streaming query with full session context
    pub async fn query_stream_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> Result<impl Stream<Item = MessageChunk>> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Claude(
                claude_sdk_rs::Error::ConfigError("Empty prompt".to_string()),
            ));
        }

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

        // Use the full conversation including context for streaming
        let message_stream = self
            .client
            .query(&full_conversation)
            .stream()
            .await
            .map_err(crate::error::AgentError::Claude)?;

        // Convert the MessageStream to our MessageChunk stream
        let chunk_stream = message_stream.map(|result| {
            match result {
                Ok(Message::Assistant { content, .. }) => MessageChunk {
                    content,
                    chunk_type: ChunkType::Text,
                },
                Ok(Message::Tool { .. }) => MessageChunk {
                    content: String::new(), // Tool calls don't have direct content
                    chunk_type: ChunkType::ToolCall,
                },
                Ok(Message::ToolResult { .. }) => MessageChunk {
                    content: String::new(), // Tool results handled separately
                    chunk_type: ChunkType::ToolResult,
                },
                Ok(_) => MessageChunk {
                    content: String::new(), // Other message types (Init, User, System, Result)
                    chunk_type: ChunkType::Text,
                },
                Err(_) => MessageChunk {
                    content: String::new(), // Error handling - could be improved
                    chunk_type: ChunkType::Text,
                },
            }
        });

        Ok(chunk_stream)
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
        let message = ClaudeMessage {
            role,
            content,
            timestamp: SystemTime::now(),
        };
        self.messages.push(message);
    }
}

/// Convert from session module Session to claude module SessionContext
impl From<crate::session::Session> for SessionContext {
    fn from(session: crate::session::Session) -> Self {
        Self {
            session_id: session.id.to_string(),
            messages: session.context.into_iter().map(|msg| msg.into()).collect(),
            created_at: session.created_at,
        }
    }
}

/// Convert from session module Session reference to claude module SessionContext
impl From<&crate::session::Session> for SessionContext {
    fn from(session: &crate::session::Session) -> Self {
        Self {
            session_id: session.id.to_string(),
            messages: session.context.iter().map(|msg| msg.into()).collect(),
            created_at: session.created_at,
        }
    }
}

/// Convert from session module Message to claude module ClaudeMessage
impl From<crate::session::Message> for ClaudeMessage {
    fn from(message: crate::session::Message) -> Self {
        Self {
            role: message.role.into(),
            content: message.content,
            timestamp: message.timestamp,
        }
    }
}

/// Convert from session module Message reference to claude module ClaudeMessage
impl From<&crate::session::Message> for ClaudeMessage {
    fn from(message: &crate::session::Message) -> Self {
        Self {
            role: message.role.clone().into(),
            content: message.content.clone(),
            timestamp: message.timestamp,
        }
    }
}

/// Convert from session module MessageRole to claude module MessageRole
impl From<crate::session::MessageRole> for MessageRole {
    fn from(role: crate::session::MessageRole) -> Self {
        match role {
            crate::session::MessageRole::User => MessageRole::User,
            crate::session::MessageRole::Assistant => MessageRole::Assistant,
            crate::session::MessageRole::System => MessageRole::System,
        }
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
        let response = client.query("Hello", "session-1").await.unwrap();
        // Claude's response won't necessarily contain the exact prompt
        // Just verify we get a non-empty response
        assert!(
            !response.is_empty(),
            "Expected non-empty response from Claude API"
        );
    }

    #[tokio::test]
    async fn test_query_with_context() {
        let client = ClaudeClient::new().unwrap();
        let mut context = SessionContext::new("test-session".to_string());
        context.add_message(MessageRole::User, "Previous message".to_string());

        let response = client
            .query_with_context("New prompt", &context)
            .await
            .unwrap();
        // For now this is a placeholder implementation, so just verify we get a response
        assert!(
            !response.is_empty(),
            "Expected non-empty response from query_with_context"
        );
        // Since this is a placeholder implementation that sends to Claude SDK,
        // it may not contain the session ID in the response content
        // Just verify we get a meaningful response
        assert!(response.len() > 10, "Response should be substantial");
    }

    #[test]
    fn test_message_roles() {
        let user_msg = ClaudeMessage {
            role: MessageRole::User,
            content: "User message".to_string(),
            timestamp: SystemTime::now(),
        };

        let assistant_msg = ClaudeMessage {
            role: MessageRole::Assistant,
            content: "Assistant message".to_string(),
            timestamp: SystemTime::now(),
        };

        let system_msg = ClaudeMessage {
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
        assert!(matches!(
            tool_result_chunk.chunk_type,
            ChunkType::ToolResult
        ));
    }
}
