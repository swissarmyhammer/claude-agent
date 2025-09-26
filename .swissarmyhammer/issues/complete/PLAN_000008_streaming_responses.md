# Streaming Response Implementation

Refer to plan.md

## Goal
Implement streaming capabilities for real-time response delivery using session/update notifications.

## Tasks

### 1. Streaming Infrastructure (`lib/src/agent.rs`)

```rust
use agent_client_protocol::{
    SessionUpdateNotification, MessageChunk, Role, ContentBlock
};
use tokio_stream::StreamExt;

impl Agent for ClaudeAgent {
    async fn session_prompt(&self, request: PromptRequest) -> crate::Result<PromptResponse> {
        self.log_request("session_prompt", &request);
        
        let session_id = uuid::Uuid::parse_str(&request.session_id)
            .map_err(|_| crate::AgentError::Session("Invalid session ID format".to_string()))?;
        
        // Validate and get session
        self.validate_prompt_request(&request).await?;
        let session = self.session_manager.get_session(&session_id)?
            .ok_or_else(|| crate::AgentError::Session("Session not found".to_string()))?;
        
        // Add user message to session
        let user_message = crate::session::Message {
            role: crate::session::MessageRole::User,
            content: request.prompt.clone(),
            timestamp: std::time::SystemTime::now(),
        };
        
        self.session_manager.update_session(&session_id, |session| {
            session.add_message(user_message);
        })?;
        
        // Check if streaming is supported and requested
        if self.should_stream(&session, &request) {
            self.handle_streaming_prompt(&session_id, &request, &session).await
        } else {
            self.handle_non_streaming_prompt(&session_id, &request, &session).await
        }
    }
    
    fn should_stream(&self, session: &crate::session::Session, _request: &PromptRequest) -> bool {
        // Check if client supports streaming
        session.client_capabilities
            .as_ref()
            .and_then(|caps| caps.streaming)
            .unwrap_or(false)
    }
}
```

### 2. Streaming Response Handler

```rust
impl ClaudeAgent {
    async fn handle_streaming_prompt(
        &self,
        session_id: &uuid::Uuid,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> crate::Result<PromptResponse> {
        tracing::info!("Handling streaming prompt for session: {}", session_id);
        
        let context: crate::claude::SessionContext = session.into();
        let mut stream = self.claude_client.query_stream_with_context(&request.prompt, &context).await?;
        
        let mut full_response = String::new();
        let mut chunk_count = 0;
        
        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    chunk_count += 1;
                    full_response.push_str(&chunk.content);
                    
                    // Send real-time update via session/update notification
                    self.send_session_update(SessionUpdateNotification {
                        session_id: request.session_id.clone(),
                        message_chunk: Some(MessageChunk {
                            role: Role::Agent,
                            content: vec![ContentBlock::Text {
                                text: chunk.content,
                            }],
                        }),
                    }).await?;
                }
                Err(e) => {
                    tracing::error!("Streaming error: {}", e);
                    // Send error update
                    self.send_error_update(&request.session_id, &e.to_string()).await?;
                    return Err(e);
                }
            }
        }
        
        tracing::info!("Completed streaming response with {} chunks", chunk_count);
        
        // Store complete response in session
        let assistant_message = crate::session::Message {
            role: crate::session::MessageRole::Assistant,
            content: full_response,
            timestamp: std::time::SystemTime::now(),
        };
        
        self.session_manager.update_session(session_id, |session| {
            session.add_message(assistant_message);
        })?;
        
        Ok(PromptResponse {
            stop_reason: StopReason::EndTurn,
            session_id: request.session_id.clone(),
        })
    }
    
    async fn handle_non_streaming_prompt(
        &self,
        session_id: &uuid::Uuid,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> crate::Result<PromptResponse> {
        tracing::info!("Handling non-streaming prompt for session: {}", session_id);
        
        let context: crate::claude::SessionContext = session.into();
        let response_content = self.claude_client.query_with_context(&request.prompt, &context).await?;
        
        // Store assistant response in session
        let assistant_message = crate::session::Message {
            role: crate::session::MessageRole::Assistant,
            content: response_content,
            timestamp: std::time::SystemTime::now(),
        };
        
        self.session_manager.update_session(session_id, |session| {
            session.add_message(assistant_message);
        })?;
        
        Ok(PromptResponse {
            stop_reason: StopReason::EndTurn,
            session_id: request.session_id.clone(),
        })
    }
}
```

### 3. Notification Sending

```rust
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct NotificationSender {
    sender: broadcast::Sender<SessionUpdateNotification>,
}

impl NotificationSender {
    pub fn new() -> (Self, broadcast::Receiver<SessionUpdateNotification>) {
        let (sender, receiver) = broadcast::channel(1000);
        (Self { sender }, receiver)
    }
    
    pub async fn send_update(&self, update: SessionUpdateNotification) -> crate::Result<()> {
        self.sender.send(update)
            .map_err(|_| crate::AgentError::Protocol("Failed to send notification".to_string()))?;
        Ok(())
    }
}

impl ClaudeAgent {
    pub fn new(config: AgentConfig) -> crate::Result<(Self, broadcast::Receiver<SessionUpdateNotification>)> {
        let (notification_sender, notification_receiver) = NotificationSender::new();
        
        let agent = Self {
            session_manager: Arc::new(SessionManager::new()),
            claude_client: Arc::new(ClaudeClient::new_with_config(&config.claude)?),
            config,
            capabilities: ServerCapabilities {
                streaming: Some(true),
                tools: Some(vec![
                    "fs_read".to_string(),
                    "fs_write".to_string(),
                    "terminal_create".to_string(),
                    "terminal_write".to_string(),
                ]),
            },
            notification_sender: Arc::new(notification_sender),
        };
        
        Ok((agent, notification_receiver))
    }
    
    async fn send_session_update(&self, update: SessionUpdateNotification) -> crate::Result<()> {
        self.notification_sender.send_update(update).await
    }
    
    async fn send_error_update(&self, session_id: &str, error_message: &str) -> crate::Result<()> {
        let update = SessionUpdateNotification {
            session_id: session_id.to_string(),
            message_chunk: Some(MessageChunk {
                role: Role::Agent,
                content: vec![ContentBlock::Text {
                    text: format!("Error: {}", error_message),
                }],
            }),
        };
        
        self.send_session_update(update).await
    }
}
```

### 4. Enhanced Claude Client Streaming

```rust
// In lib/src/claude.rs - implement actual streaming

use tokio_stream::{Stream, wrappers::ReceiverStream};
use tokio::sync::mpsc;

impl ClaudeClient {
    pub async fn query_stream_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> crate::Result<impl Stream<Item = Result<crate::claude::MessageChunk, crate::AgentError>>> {
        tracing::info!("Starting streaming query with {} context messages", context.messages.len());
        
        let (tx, rx) = mpsc::channel(100);
        
        // For now, simulate streaming by chunking the response
        let response = self.query_with_context(prompt, context).await?;
        
        tokio::spawn(async move {
            // Simulate streaming by sending chunks
            let words: Vec<&str> = response.split_whitespace().collect();
            
            for chunk in words.chunks(3) {
                let chunk_text = chunk.join(" ") + " ";
                let message_chunk = crate::claude::MessageChunk {
                    content: chunk_text,
                    chunk_type: crate::claude::ChunkType::Text,
                };
                
                if tx.send(Ok(message_chunk)).await.is_err() {
                    break; // Receiver dropped
                }
                
                // Small delay to simulate streaming
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        });
        
        Ok(ReceiverStream::new(rx))
    }
}
```

### 5. Error Handling and Recovery

```rust
impl ClaudeAgent {
    async fn handle_streaming_error(
        &self,
        session_id: &str,
        error: &crate::AgentError,
    ) -> crate::Result<()> {
        tracing::error!("Streaming error in session {}: {}", session_id, error);
        
        // Send error notification to client
        self.send_error_update(session_id, &error.to_string()).await?;
        
        // Optionally, attempt recovery or cleanup
        match error {
            crate::AgentError::Claude(_) => {
                // Claude API error - might be temporary
                tracing::info!("Claude API error, client should retry");
            }
            crate::AgentError::Protocol(_) => {
                // Protocol error - likely permanent
                tracing::warn!("Protocol error, client should check request");
            }
            _ => {
                // Other errors
                tracing::debug!("Handling other error types");
            }
        }
        
        Ok(())
    }
}
```

### 6. Integration Tests

```rust
#[cfg(test)]
mod streaming_tests {
    use super::*;
    use tokio_stream::StreamExt;
    
    #[tokio::test]
    async fn test_streaming_prompt() {
        let (agent, mut notification_receiver) = create_test_agent_with_notifications();
        
        // Create session with streaming capabilities
        let mut client_capabilities = ClientCapabilities::default();
        client_capabilities.streaming = Some(true);
        
        let new_session_request = SessionNewRequest {
            client_capabilities: Some(client_capabilities),
        };
        let session_response = agent.session_new(new_session_request).await.unwrap();
        
        // Send streaming prompt
        let prompt_request = PromptRequest {
            session_id: session_response.session_id.clone(),
            prompt: "Tell me a story".to_string(),
        };
        
        // Start prompt in background
        let agent_clone = Arc::new(agent);
        let prompt_task = tokio::spawn({
            let agent = Arc::clone(&agent_clone);
            async move {
                agent.session_prompt(prompt_request).await
            }
        });
        
        // Collect streaming updates
        let mut updates = Vec::new();
        let timeout = tokio::time::sleep(std::time::Duration::from_secs(5));
        tokio::pin!(timeout);
        
        loop {
            tokio::select! {
                update = notification_receiver.recv() => {
                    match update {
                        Ok(notification) => {
                            if notification.session_id == session_response.session_id {
                                updates.push(notification);
                                // Break after receiving some updates
                                if updates.len() >= 3 {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                _ = &mut timeout => {
                    panic!("Timeout waiting for streaming updates");
                }
            }
        }
        
        // Wait for prompt to complete
        let prompt_result = prompt_task.await.unwrap();
        assert!(prompt_result.is_ok());
        
        // Verify we received streaming updates
        assert!(!updates.is_empty());
        assert!(updates.iter().all(|u| u.message_chunk.is_some()));
    }
    
    #[tokio::test]
    async fn test_non_streaming_fallback() {
        let (agent, _) = create_test_agent_with_notifications();
        
        // Create session without streaming capabilities
        let new_session_request = SessionNewRequest {
            client_capabilities: None,
        };
        let session_response = agent.session_new(new_session_request).await.unwrap();
        
        let prompt_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: "Hello".to_string(),
        };
        
        let result = agent.session_prompt(prompt_request).await;
        assert!(result.is_ok());
    }
    
    fn create_test_agent_with_notifications() -> (ClaudeAgent, broadcast::Receiver<SessionUpdateNotification>) {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).unwrap()
    }
}
```

## Files Modified
- `lib/src/agent.rs` - Add streaming prompt handling and notifications
- `lib/src/claude.rs` - Implement streaming query methods
- Add streaming integration tests

## Acceptance Criteria
- Streaming responses work when client supports it
- Non-streaming fallback works for clients without streaming support
- Session updates are sent via notifications during streaming
- Error handling includes streaming error recovery
- Full response is stored in session after streaming completes
- Integration tests verify streaming and non-streaming flows
- `cargo build` and `cargo test` succeed

## Proposed Solution

After analyzing the current codebase, I've identified the approach to implement streaming responses:

### Current State Analysis
- `ClaudeAgent` in `agent.rs` implements the `Agent` trait but the `prompt` method doesn't support streaming
- `ClaudeClient` in `claude.rs` already has streaming infrastructure with `MessageChunk` and `ChunkType`
- Session management in `session.rs` includes `client_capabilities` that can indicate streaming support
- Required dependencies (`tokio-stream`, `broadcast`) are already available

### Implementation Steps

1. **Add Notification System to ClaudeAgent**
   - Add `NotificationSender` struct with broadcast channels
   - Modify `ClaudeAgent::new()` to return `(Self, broadcast::Receiver<SessionUpdateNotification>)`
   - Add methods for sending session updates and error notifications

2. **Enhance Prompt Method**
   - Replace current `prompt` method implementation with routing logic
   - Check session's `client_capabilities.streaming` to determine if streaming is supported
   - Route to either `handle_streaming_prompt` or `handle_non_streaming_prompt`

3. **Streaming Handler Implementation**
   - Use existing `query_stream_with_context` from `ClaudeClient`
   - Process chunks in real-time, sending `SessionUpdateNotification` via broadcast channel
   - Accumulate full response and store in session after streaming completes
   - Handle streaming errors gracefully with error notifications

4. **Non-Streaming Handler**
   - Use existing `query_with_context` from `ClaudeClient` (current behavior)
   - Maintain backward compatibility for clients without streaming support

5. **Error Handling**
   - Implement `handle_streaming_error` for streaming-specific error recovery
   - Send error updates via notifications when streaming fails
   - Proper cleanup and session state management on errors

6. **Integration Tests**
   - Test streaming flow with client that supports streaming
   - Test non-streaming fallback for clients without streaming support
   - Test error scenarios and recovery

### Key Design Decisions
- Use existing `MessageChunk` and streaming infrastructure from `claude.rs`
- Leverage session's `client_capabilities` for capability detection
- Maintain backward compatibility with non-streaming clients
- Use broadcast channels for real-time notifications
- Store complete response in session after streaming (for consistency)

This approach builds on the existing architecture while adding streaming capabilities in a clean, backward-compatible way.
## Implementation Complete ✅

The streaming response implementation has been successfully completed and tested. Here's a summary of what was implemented:

### What Was Implemented

#### 1. **NotificationSender Infrastructure**
- Added `SessionUpdateNotification` and `MessageChunk` types for streaming updates
- Created `NotificationSender` with broadcast channel for real-time notifications
- Modified `ClaudeAgent::new()` to return `(Self, broadcast::Receiver<SessionUpdateNotification>)`

#### 2. **Streaming Detection Logic** 
- Implemented `should_stream()` method that checks session's `client_capabilities.meta.streaming`
- Routes to appropriate handler based on client streaming support

#### 3. **Dual Prompt Handling**
- **Streaming Handler**: Uses existing `query_stream_with_context()` from Claude client
  - Processes chunks in real-time sending notifications via broadcast channel
  - Accumulates full response and stores in session after completion
  - Handles streaming errors gracefully
- **Non-Streaming Handler**: Uses existing `query_with_context()` (maintains backward compatibility)

#### 4. **Enhanced Agent Capabilities**
- Updated agent metadata to indicate streaming support: `"streaming": true`
- Maintains full backward compatibility with non-streaming clients

### Files Modified
- ✅ **`lib/src/agent.rs`**: Added streaming infrastructure, notification system, and dual prompt handlers
- ✅ **No changes needed to `lib/src/claude.rs`**: Existing streaming infrastructure was already complete
- ✅ **No changes needed to `Cargo.toml`**: Required dependencies already available

### Tests Added
- ✅ **`test_streaming_prompt`**: Verifies streaming works when client supports it
- ✅ **`test_non_streaming_fallback`**: Verifies fallback for clients without streaming support  
- ✅ **`test_streaming_capability_detection`**: Tests the `should_stream()` logic
- ✅ **`test_streaming_session_context_maintained`**: Ensures conversation context is preserved across streaming requests

### Build & Test Status
- ✅ **`cargo build`**: Compiles successfully
- ✅ **`cargo nextest run`**: All 65 tests pass (including 4 new streaming integration tests)

### Key Design Features
- **Backward Compatible**: Clients without streaming support continue to work unchanged
- **Capability-Based**: Uses session's `client_capabilities.meta.streaming` for detection  
- **Real-Time Updates**: Broadcast channel sends notifications as chunks arrive
- **Complete Response Storage**: Full response stored in session after streaming completes
- **Error Handling**: Graceful error recovery with notification system
- **Session Context**: Conversation history maintained across streaming and non-streaming requests

The implementation fully satisfies all acceptance criteria and is ready for use.