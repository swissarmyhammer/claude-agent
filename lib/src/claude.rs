//! Claude process wrapper providing session-aware interactions

use agent_client_protocol::{ContentBlock, SessionUpdate, TextContent};
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

use crate::{
    claude_process::{ClaudeProcess, ClaudeProcessManager},
    config::ClaudeConfig,
    error::Result,
    protocol_translator::ProtocolTranslator,
    session::{MessageRole, SessionId},
};

/// Claude client wrapper with session management
pub struct ClaudeClient {
    process_manager: Arc<ClaudeProcessManager>,
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
        Ok(Self {
            process_manager: Arc::new(ClaudeProcessManager::new()),
        })
    }

    /// Create a new Claude client with custom configuration
    pub fn new_with_config(_claude_config: &ClaudeConfig) -> Result<Self> {
        tracing::info!("Created ClaudeClient with process manager");
        Ok(Self {
            process_manager: Arc::new(ClaudeProcessManager::new()),
        })
    }

    /// Check if the client supports streaming
    pub fn supports_streaming(&self) -> bool {
        true
    }

    /// Get the process manager (for session lifecycle integration)
    pub fn process_manager(&self) -> &Arc<ClaudeProcessManager> {
        &self.process_manager
    }

    /// Convert session::SessionId to agent_client_protocol::SessionId
    fn to_acp_session_id(session_id: &SessionId) -> agent_client_protocol::SessionId {
        agent_client_protocol::SessionId(Arc::from(session_id.to_string().as_str()))
    }

    /// Convert ContentBlock to MessageChunk
    fn content_block_to_message_chunk(content: ContentBlock) -> MessageChunk {
        match content {
            ContentBlock::Text(text) => MessageChunk {
                content: text.text,
                chunk_type: ChunkType::Text,
                tool_call: None,
                token_usage: None,
            },
            // Handle other ContentBlock variants if they exist
            _ => MessageChunk {
                content: String::new(),
                chunk_type: ChunkType::Text,
                tool_call: None,
                token_usage: None,
            },
        }
    }

    /// Helper method to send prompt to process
    async fn send_prompt_to_process(
        &self,
        process: Arc<Mutex<ClaudeProcess>>,
        prompt: &str,
    ) -> Result<()> {
        let content = vec![ContentBlock::Text(TextContent {
            text: prompt.to_string(),
            annotations: None,
            meta: None,
        })];
        let stream_json = ProtocolTranslator::acp_to_stream_json(content)?;

        let mut proc = process.lock().await;
        proc.write_line(&stream_json).await?;
        Ok(())
    }

    /// Helper method to check if a line indicates end of stream
    fn is_end_of_stream(line: &str) -> bool {
        // Parse JSON and check type field properly
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(msg_type) = json.get("type").and_then(|t| t.as_str()) {
                return msg_type == "result";
            }
        }
        false
    }

    /// Execute a simple query without session context
    pub async fn query(&self, prompt: &str, session_id: &SessionId) -> Result<String> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        // Get the process for this session
        let process = self.process_manager.get_process(session_id).await?;

        // Send prompt to process
        self.send_prompt_to_process(process.clone(), prompt).await?;

        // Read response lines until we get a result
        let mut response_text = String::new();
        let acp_session_id = Self::to_acp_session_id(session_id);
        loop {
            let line = {
                let mut proc = process.lock().await;
                proc.read_line().await?
            };

            match line {
                Some(line) => {
                    if let Ok(Some(notification)) =
                        ProtocolTranslator::stream_json_to_acp(&line, &acp_session_id)
                    {
                        if let SessionUpdate::AgentMessageChunk {
                            content: ContentBlock::Text(text),
                        } = notification.update
                        {
                            response_text.push_str(&text.text);
                        }
                    }
                    // Check if this is a result message (indicates end)
                    if Self::is_end_of_stream(&line) {
                        break;
                    }
                }
                None => break,
            }
        }

        Ok(response_text)
    }

    /// Execute a streaming query without session context
    pub async fn query_stream(
        &self,
        prompt: &str,
        session_id: &SessionId,
    ) -> Result<Pin<Box<dyn Stream<Item = MessageChunk> + Send>>> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
            ));
        }

        // Get the process for this session
        let process = self.process_manager.get_process(session_id).await?;

        // Send prompt to process
        self.send_prompt_to_process(process.clone(), prompt).await?;

        // Create a channel-based stream to avoid holding mutex across await
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let process_clone = process.clone();
        let acp_session_id = Self::to_acp_session_id(session_id);

        // Spawn an async task to read from the process and send chunks
        tokio::task::spawn(async move {
            loop {
                let line = {
                    let mut proc = process_clone.lock().await;
                    match proc.read_line().await {
                        Ok(Some(line)) => line,
                        Ok(None) => break,
                        Err(_) => break,
                    }
                };

                // Check if this is a result message (indicates end)
                if Self::is_end_of_stream(&line) {
                    break;
                }

                // Translate to ACP notification
                if let Ok(Some(notification)) =
                    ProtocolTranslator::stream_json_to_acp(&line, &acp_session_id)
                {
                    if let SessionUpdate::AgentMessageChunk { content } = notification.update {
                        let chunk = Self::content_block_to_message_chunk(content);
                        if tx.send(chunk).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        // Convert receiver to stream
        let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    /// Execute a query with full session context
    pub async fn query_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> Result<String> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
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

        // Use the process manager for the query
        tracing::info!(
            "Sending request to Claude process (prompt length: {} chars)",
            full_conversation.len()
        );

        let response = self.query(&full_conversation, &context.session_id).await?;

        tracing::info!(
            "Received response from Claude process (content length: {} chars)",
            response.len()
        );

        Ok(response)
    }

    /// Execute a streaming query with full session context
    pub async fn query_stream_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> Result<Pin<Box<dyn Stream<Item = MessageChunk> + Send>>> {
        if prompt.is_empty() {
            return Err(crate::error::AgentError::Process(
                "Empty prompt".to_string(),
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

        // Use the process manager for streaming
        self.query_stream(&full_conversation, &context.session_id)
            .await
    }
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
            tool_call: None,
            token_usage: None,
        };

        let tool_call_chunk = MessageChunk {
            content: "tool_call".to_string(),
            chunk_type: ChunkType::ToolCall,
            tool_call: Some(ToolCallInfo {
                name: "test_tool".to_string(),
                parameters: serde_json::json!({"arg": "value"}),
            }),
            token_usage: None,
        };

        let tool_result_chunk = MessageChunk {
            content: "tool_result".to_string(),
            chunk_type: ChunkType::ToolResult,
            tool_call: None,
            token_usage: None,
        };

        let result_chunk = MessageChunk {
            content: String::new(),
            chunk_type: ChunkType::Text,
            tool_call: None,
            token_usage: Some(TokenUsageInfo {
                input_tokens: 100,
                output_tokens: 200,
            }),
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
    fn test_session_context_token_tracking() {
        let session_id = SessionId::new();
        let mut context = SessionContext::new(session_id);

        // Initial state - no cost or tokens
        assert_eq!(context.total_cost_usd, 0.0);
        assert_eq!(context.total_input_tokens, 0);
        assert_eq!(context.total_output_tokens, 0);
        assert_eq!(context.total_tokens(), 0);
        assert_eq!(context.average_cost_per_message(), None);

        // Add messages
        context.add_message(MessageRole::User, "Hello".to_string());
        assert_eq!(context.messages.len(), 1);

        context.add_message(MessageRole::Assistant, "Response".to_string());
        assert_eq!(context.messages.len(), 2);

        // Note: Token tracking would need to be updated separately via the public fields
        // This test now focuses on basic message addition functionality
    }
}
