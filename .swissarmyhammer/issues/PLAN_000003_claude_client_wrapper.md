# Claude SDK Integration

Refer to plan.md

## Goal
Create a wrapper around claude-sdk-rs to provide session-aware Claude interactions with streaming support.

## Tasks

### 1. Claude Client Wrapper (`lib/src/claude.rs`)

```rust
use claude_sdk_rs::{Client, Config, Error as ClaudeError};
use tokio_stream::Stream;
use serde_json::Value;

pub struct ClaudeClient {
    client: Client,
    config: Config,
}

impl ClaudeClient {
    pub fn new() -> crate::Result<Self> {
        let config = Config::default();
        let client = Client::new(config.clone())?;
        
        Ok(Self { client, config })
    }
    
    pub async fn query(&self, prompt: &str, session_id: &str) -> crate::Result<String> {
        // Non-streaming query implementation
        // Include session context in the request
        todo!()
    }
    
    pub async fn query_stream(
        &self, 
        prompt: &str, 
        session_id: &str
    ) -> crate::Result<impl Stream<Item = MessageChunk>> {
        // Streaming query implementation
        // Return stream of message chunks
        todo!()
    }
    
    pub fn supports_streaming(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct MessageChunk {
    pub content: String,
    pub chunk_type: ChunkType,
}

#[derive(Debug, Clone)]
pub enum ChunkType {
    Text,
    ToolCall,
    ToolResult,
}
```

### 2. Session Context Management

```rust
use std::collections::HashMap;

pub struct SessionContext {
    pub session_id: String,
    pub messages: Vec<Message>,
    pub created_at: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl ClaudeClient {
    pub async fn query_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> crate::Result<String> {
        // Build conversation history from context
        // Send to Claude with full conversation
        todo!()
    }
    
    pub async fn query_stream_with_context(
        &self,
        prompt: &str, 
        context: &SessionContext,
    ) -> crate::Result<impl Stream<Item = MessageChunk>> {
        // Streaming version with context
        todo!()
    }
}
```

### 3. Error Handling and Retry Logic

```rust
use tokio::time::{sleep, Duration};

impl ClaudeClient {
    async fn execute_with_retry<F, T>(&self, operation: F) -> crate::Result<T> 
    where
        F: Fn() -> futures::future::BoxFuture<'_, crate::Result<T>>,
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

fn is_retryable(error: &crate::AgentError) -> bool {
    // Determine if error is worth retrying
    matches!(error, crate::AgentError::Claude(_))
}
```

### 4. Configuration Integration

```rust
impl ClaudeClient {
    pub fn new_with_config(config: &crate::config::ClaudeConfig) -> crate::Result<Self> {
        let sdk_config = Config {
            model: config.model.clone(),
            // Map other config fields
            ..Config::default()
        };
        
        let client = Client::new(sdk_config.clone())?;
        Ok(Self { client, config: sdk_config })
    }
}
```

### 5. Unit Tests

```rust
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
        let context = SessionContext {
            session_id: "test-session".to_string(),
            messages: vec![],
            created_at: std::time::SystemTime::now(),
        };
        
        // Test context handling
    }
    
    // Add more tests for error handling, retry logic, etc.
}
```

## Files Created
- `lib/src/claude.rs` - Claude client wrapper with streaming support
- Update `lib/src/lib.rs` to export claude module

## Acceptance Criteria
- ClaudeClient can be created with default and custom configs
- Non-streaming queries work (may use mock for tests)
- Streaming interface is defined (implementation can be placeholder)
- Session context is properly maintained
- Error handling includes retry logic
- Unit tests pass
- `cargo build` and `cargo test` succeed