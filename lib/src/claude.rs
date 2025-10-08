//! Claude SDK wrapper providing session-aware interactions

use claude_sdk_rs::{Client, Config, Message, MessageMeta};
use tokio::time::{sleep, Duration};
use tokio_stream::{Stream, StreamExt};

use std::time::SystemTime;

use crate::{
    config::ClaudeConfig,
    error::Result,
    session::{MessageRole, SessionId},
};

/// Claude client wrapper with session management
pub struct ClaudeClient {
    client: Client,
}

/// Session context for managing conversation history
pub struct SessionContext {
    pub session_id: SessionId,
    pub messages: Vec<ClaudeMessage>,
    pub created_at: SystemTime,
    /// Total cost in USD for all messages in this session
    pub total_cost_usd: f64,
    /// Total input tokens used across all messages
    pub total_input_tokens: u64,
    /// Total output tokens used across all messages
    pub total_output_tokens: u64,
}

/// Individual message in a conversation
#[derive(Debug, Clone)]
pub struct ClaudeMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: SystemTime,
    /// Optional metadata from Claude SDK including cost, tokens, and duration
    pub meta: Option<MessageMeta>,
}

/// Streaming message chunk
#[derive(Debug, Clone)]
pub struct MessageChunk {
    pub content: String,
    pub chunk_type: ChunkType,
    /// Tool call information (only present when chunk_type is ToolCall)
    pub tool_call: Option<ToolCallInfo>,
    /// Token usage information (only present in Result messages)
    pub token_usage: Option<TokenUsageInfo>,
    /// Optional metadata from SDK Message including cost, tokens, and duration
    pub meta: Option<MessageMeta>,
}

/// Tool call information extracted from Message::Tool
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub parameters: serde_json::Value,
}

/// Token usage information extracted from Message metadata
#[derive(Debug, Clone)]
pub struct TokenUsageInfo {
    pub input_tokens: u64,
    pub output_tokens: u64,
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
        let config = Config::builder()
            .timeout_secs(300) // 5 minute timeout for Claude API calls
            .build()?;
        let client = Client::new(config);

        Ok(Self { client })
    }

    /// Create a new Claude client with custom configuration
    pub fn new_with_config(_claude_config: &ClaudeConfig) -> Result<Self> {
        let config = Config::builder()
            .timeout_secs(300) // 5 minute timeout for Claude API calls
            .build()?;

        tracing::info!(
            "Created ClaudeClient with timeout_secs: {:?}",
            config.timeout_secs
        );

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
    pub async fn query(&self, prompt: &str, _session_id: &SessionId) -> Result<String> {
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
        _session_id: &SessionId,
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
                Ok(Message::Assistant { content, meta }) => MessageChunk {
                    content,
                    chunk_type: ChunkType::Text,
                    tool_call: None,
                    token_usage: None,
                    meta: Some(meta),
                },
                Ok(Message::Tool {
                    name,
                    parameters,
                    meta,
                }) => MessageChunk {
                    content: String::new(), // Tool calls don't have direct content
                    chunk_type: ChunkType::ToolCall,
                    tool_call: Some(ToolCallInfo { name, parameters }),
                    token_usage: None,
                    meta: Some(meta),
                },
                Ok(Message::ToolResult { meta, .. }) => MessageChunk {
                    content: String::new(), // Tool results handled separately
                    chunk_type: ChunkType::ToolResult,
                    tool_call: None,
                    token_usage: None,
                    meta: Some(meta),
                },
                Ok(Message::Result { meta, .. }) => {
                    // Extract token usage from metadata for backward compatibility
                    let token_usage = meta.tokens_used.as_ref().map(|tokens| TokenUsageInfo {
                        input_tokens: tokens.input,
                        output_tokens: tokens.output,
                    });
                    MessageChunk {
                        content: String::new(),
                        chunk_type: ChunkType::Text,
                        tool_call: None,
                        token_usage,
                        meta: Some(meta),
                    }
                }
                Ok(msg) => {
                    // Other message types (Init, User, System) - extract meta if available
                    let meta = match msg {
                        Message::Init { meta } => Some(meta),
                        Message::User { meta, .. } => Some(meta),
                        Message::System { meta, .. } => Some(meta),
                        _ => None,
                    };
                    MessageChunk {
                        content: String::new(),
                        chunk_type: ChunkType::Text,
                        tool_call: None,
                        token_usage: None,
                        meta,
                    }
                }
                Err(_) => MessageChunk {
                    content: String::new(), // Error handling - could be improved
                    chunk_type: ChunkType::Text,
                    tool_call: None,
                    token_usage: None,
                    meta: None,
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
            tracing::info!(
                "Sending request to Claude SDK (prompt length: {} chars)",
                full_conversation.len()
            );
            let response = self
                .client
                .send_full(&full_conversation)
                .await
                .map_err(|e| {
                    tracing::error!("Claude SDK error: {:?}", e);
                    crate::error::AgentError::Claude(e)
                })?;
            tracing::info!(
                "Received response from Claude SDK (content length: {} chars)",
                response.content.len()
            );

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
                Ok(Message::Assistant { content, meta }) => MessageChunk {
                    content,
                    chunk_type: ChunkType::Text,
                    tool_call: None,
                    token_usage: None,
                    meta: Some(meta),
                },
                Ok(Message::Tool {
                    name,
                    parameters,
                    meta,
                }) => MessageChunk {
                    content: String::new(), // Tool calls don't have direct content
                    chunk_type: ChunkType::ToolCall,
                    tool_call: Some(ToolCallInfo { name, parameters }),
                    token_usage: None,
                    meta: Some(meta),
                },
                Ok(Message::ToolResult { meta, .. }) => MessageChunk {
                    content: String::new(), // Tool results handled separately
                    chunk_type: ChunkType::ToolResult,
                    tool_call: None,
                    token_usage: None,
                    meta: Some(meta),
                },
                Ok(Message::Result { meta, .. }) => {
                    // Extract token usage from metadata for backward compatibility
                    let token_usage = meta.tokens_used.as_ref().map(|tokens| TokenUsageInfo {
                        input_tokens: tokens.input,
                        output_tokens: tokens.output,
                    });
                    MessageChunk {
                        content: String::new(),
                        chunk_type: ChunkType::Text,
                        tool_call: None,
                        token_usage,
                        meta: Some(meta),
                    }
                }
                Ok(msg) => {
                    // Other message types (Init, User, System) - extract meta if available
                    let meta = match msg {
                        Message::Init { meta } => Some(meta),
                        Message::User { meta, .. } => Some(meta),
                        Message::System { meta, .. } => Some(meta),
                        _ => None,
                    };
                    MessageChunk {
                        content: String::new(),
                        chunk_type: ChunkType::Text,
                        tool_call: None,
                        token_usage: None,
                        meta,
                    }
                }
                Err(_) => MessageChunk {
                    content: String::new(), // Error handling - could be improved
                    chunk_type: ChunkType::Text,
                    tool_call: None,
                    token_usage: None,
                    meta: None,
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
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            messages: Vec::new(),
            created_at: SystemTime::now(),
            total_cost_usd: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        let message = ClaudeMessage {
            role,
            content,
            timestamp: SystemTime::now(),
            meta: None,
        };
        self.messages.push(message);
    }

    /// Add a message to the session with metadata
    pub fn add_message_with_meta(&mut self, role: MessageRole, content: String, meta: MessageMeta) {
        // Aggregate cost and token usage from metadata
        if let Some(cost) = meta.cost_usd {
            self.total_cost_usd += cost;
        }
        if let Some(ref tokens) = meta.tokens_used {
            self.total_input_tokens += tokens.input;
            self.total_output_tokens += tokens.output;
        }

        let message = ClaudeMessage {
            role,
            content,
            timestamp: SystemTime::now(),
            meta: Some(meta),
        };
        self.messages.push(message);
    }

    /// Get total tokens used (input + output)
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }

    /// Get the average cost per message (if any messages have been added)
    pub fn average_cost_per_message(&self) -> Option<f64> {
        if self.messages.is_empty() {
            None
        } else {
            Some(self.total_cost_usd / self.messages.len() as f64)
        }
    }
}

/// Convert from session module Session to claude module SessionContext
impl From<crate::session::Session> for SessionContext {
    fn from(session: crate::session::Session) -> Self {
        Self {
            session_id: session.id,
            messages: session.context.into_iter().map(|msg| msg.into()).collect(),
            created_at: session.created_at,
            total_cost_usd: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }
}

/// Convert from session module Session reference to claude module SessionContext
impl From<&crate::session::Session> for SessionContext {
    fn from(session: &crate::session::Session) -> Self {
        Self {
            session_id: session.id,
            messages: session.context.iter().map(|msg| msg.into()).collect(),
            created_at: session.created_at,
            total_cost_usd: 0.0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }
}

/// Convert from session module Message to claude module ClaudeMessage
impl From<crate::session::Message> for ClaudeMessage {
    fn from(message: crate::session::Message) -> Self {
        Self {
            role: message.role,
            content: message.content,
            timestamp: message.timestamp,
            meta: None, // Session messages don't have SDK metadata
        }
    }
}

/// Convert from session module Message reference to claude module ClaudeMessage
impl From<&crate::session::Message> for ClaudeMessage {
    fn from(message: &crate::session::Message) -> Self {
        Self {
            role: message.role.clone(),
            content: message.content.clone(),
            timestamp: message.timestamp,
            meta: None, // Session messages don't have SDK metadata
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
        let session_id = SessionId::new();
        let mut context = SessionContext::new(session_id);
        assert_eq!(context.session_id, session_id);
        assert_eq!(context.messages.len(), 0);

        context.add_message(MessageRole::User, "Hello".to_string());
        assert_eq!(context.messages.len(), 1);
        assert!(matches!(context.messages[0].role, MessageRole::User));
        assert_eq!(context.messages[0].content, "Hello");
    }

    // NOTE: This test makes a real API call to Claude and costs money.
    // This is intentional - we want to verify actual SDK integration works.
    #[tokio::test]
    async fn test_basic_query() {
        let client = ClaudeClient::new().unwrap();
        let session_id = SessionId::new();
        let response = client.query("Hello", &session_id).await.unwrap();
        // Claude's response won't necessarily contain the exact prompt
        // Just verify we get a non-empty response
        assert!(
            !response.is_empty(),
            "Expected non-empty response from Claude API"
        );
    }

    // NOTE: This test makes a real API call to Claude and costs money.
    // This is intentional - we want to verify actual SDK integration works.
    #[tokio::test]
    async fn test_query_with_context() {
        let client = ClaudeClient::new().unwrap();
        let session_id = SessionId::new();
        let mut context = SessionContext::new(session_id);
        context.add_message(MessageRole::User, "Previous message".to_string());

        let response = client
            .query_with_context("New prompt", &context)
            .await
            .unwrap();
        // Verify we get a non-empty response from Claude SDK
        assert!(
            !response.is_empty(),
            "Expected non-empty response from query_with_context"
        );
        // Verify we get a meaningful response
        assert!(response.len() > 10, "Response should be substantial");
    }

    #[test]
    fn test_message_roles() {
        let user_msg = ClaudeMessage {
            role: MessageRole::User,
            content: "User message".to_string(),
            timestamp: SystemTime::now(),
            meta: None,
        };

        let assistant_msg = ClaudeMessage {
            role: MessageRole::Assistant,
            content: "Assistant message".to_string(),
            timestamp: SystemTime::now(),
            meta: None,
        };

        let system_msg = ClaudeMessage {
            role: MessageRole::System,
            content: "System message".to_string(),
            timestamp: SystemTime::now(),
            meta: None,
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
            tool_call: None,
            token_usage: None,
            meta: None,
        };

        let tool_call_chunk = MessageChunk {
            content: "tool_call".to_string(),
            chunk_type: ChunkType::ToolCall,
            tool_call: Some(ToolCallInfo {
                name: "test_tool".to_string(),
                parameters: serde_json::json!({"arg": "value"}),
            }),
            token_usage: None,
            meta: None,
        };

        let tool_result_chunk = MessageChunk {
            content: "tool_result".to_string(),
            chunk_type: ChunkType::ToolResult,
            tool_call: None,
            token_usage: None,
            meta: None,
        };

        let result_chunk = MessageChunk {
            content: String::new(),
            chunk_type: ChunkType::Text,
            tool_call: None,
            token_usage: Some(TokenUsageInfo {
                input_tokens: 100,
                output_tokens: 200,
            }),
            meta: None,
        };

        assert!(matches!(text_chunk.chunk_type, ChunkType::Text));
        assert!(matches!(tool_call_chunk.chunk_type, ChunkType::ToolCall));
        assert!(matches!(
            tool_result_chunk.chunk_type,
            ChunkType::ToolResult
        ));
        assert!(tool_call_chunk.tool_call.is_some());
        assert_eq!(
            tool_call_chunk.tool_call.as_ref().unwrap().name,
            "test_tool"
        );
        assert!(result_chunk.token_usage.is_some());
        assert_eq!(result_chunk.token_usage.as_ref().unwrap().input_tokens, 100);
        assert_eq!(
            result_chunk.token_usage.as_ref().unwrap().output_tokens,
            200
        );
    }

    #[test]
    fn test_session_context_metadata_aggregation() {
        use claude_sdk_rs::TokenUsage;

        let session_id = SessionId::new();
        let mut context = SessionContext::new(session_id);

        // Initial state - no cost or tokens
        assert_eq!(context.total_cost_usd, 0.0);
        assert_eq!(context.total_input_tokens, 0);
        assert_eq!(context.total_output_tokens, 0);
        assert_eq!(context.total_tokens(), 0);
        assert_eq!(context.average_cost_per_message(), None);

        // Add message with metadata
        let meta1 = MessageMeta {
            session_id: "test-session".to_string(),
            timestamp: Some(SystemTime::now()),
            cost_usd: Some(0.0015),
            duration_ms: Some(1200),
            tokens_used: Some(TokenUsage {
                input: 50,
                output: 100,
                total: 150,
            }),
        };
        context.add_message_with_meta(MessageRole::User, "Hello".to_string(), meta1);

        // Check aggregated values
        assert_eq!(context.total_cost_usd, 0.0015);
        assert_eq!(context.total_input_tokens, 50);
        assert_eq!(context.total_output_tokens, 100);
        assert_eq!(context.total_tokens(), 150);
        assert_eq!(context.average_cost_per_message(), Some(0.0015));

        // Add another message with metadata
        let meta2 = MessageMeta {
            session_id: "test-session".to_string(),
            timestamp: Some(SystemTime::now()),
            cost_usd: Some(0.0025),
            duration_ms: Some(800),
            tokens_used: Some(TokenUsage {
                input: 30,
                output: 200,
                total: 230,
            }),
        };
        context.add_message_with_meta(MessageRole::Assistant, "Response".to_string(), meta2);

        // Check cumulative values
        assert_eq!(context.total_cost_usd, 0.0040);
        assert_eq!(context.total_input_tokens, 80);
        assert_eq!(context.total_output_tokens, 300);
        assert_eq!(context.total_tokens(), 380);
        assert_eq!(context.average_cost_per_message(), Some(0.0020));

        // Add message without metadata - should not affect totals
        context.add_message(MessageRole::User, "No metadata".to_string());

        assert_eq!(context.total_cost_usd, 0.0040);
        assert_eq!(context.total_input_tokens, 80);
        assert_eq!(context.total_output_tokens, 300);
        assert_eq!(context.messages.len(), 3);
    }
}
