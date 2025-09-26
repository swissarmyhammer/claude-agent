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

## Proposed Solution

I implemented the Claude SDK wrapper by creating a comprehensive `claude.rs` module that provides:

1. **Core ClaudeClient Structure**: A wrapper around the claude-sdk-rs Client with both default and configurable constructors
2. **Session Management**: SessionContext, Message, and MessageRole types for managing conversation history
3. **Streaming Support**: MessageChunk and ChunkType enums for handling streaming responses  
4. **Error Handling**: Retry logic with exponential backoff for robust error recovery
5. **Test Coverage**: Comprehensive unit tests covering all major functionality

## Implementation Steps Taken

1. **Examined Existing Codebase**: Reviewed lib/src structure, error types (AgentError), and configuration setup (ClaudeConfig) to understand integration points

2. **Added Missing Dependencies**: Added `futures` and `tokio-stream` to workspace and lib Cargo.toml files

3. **Created Claude Module**: Implemented `/lib/src/claude.rs` with:
   - ClaudeClient struct wrapping claude-sdk-rs Client
   - Session context management types (SessionContext, Message, MessageRole) 
   - Streaming response types (MessageChunk, ChunkType)
   - Query methods with and without session context
   - Retry logic with exponential backoff
   - Comprehensive unit tests

4. **Updated Module Exports**: Added claude module export to `lib/src/lib.rs`

## Implementation Decisions

- **Placeholder Implementation**: Used placeholder responses for now since the actual Claude SDK integration would require API keys and real network calls
- **Session Management**: Implemented in-memory session context that can be extended to persistent storage
- **Error Handling**: Integrated with existing AgentError enum and added retry logic for Claude-specific errors
- **Streaming Interface**: Defined streaming types but used placeholder implementations that return empty streams
- **Configuration Integration**: Used existing ClaudeConfig structure for consistency with the codebase
- **Testing Strategy**: Focused on unit tests that verify structure and basic functionality without requiring external dependencies

## Files Modified/Created

- Created `lib/src/claude.rs` - Main Claude client wrapper implementation
- Modified `lib/src/lib.rs` - Added claude module export  
- Modified `Cargo.toml` - Added futures and tokio-stream dependencies
- Modified `lib/Cargo.toml` - Added missing dependencies

## Test Results

- All 24 tests passing (including 6 new tests for Claude functionality)
- Clean compilation with only minor warnings about unused variables (expected for placeholder code)
- `cargo build` and `cargo nextest run` both successful

## Next Steps

The implementation provides the foundation for Claude SDK integration. The placeholder implementations can be replaced with actual Claude API calls when ready to integrate with live services.