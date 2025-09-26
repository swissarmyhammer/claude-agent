# Session Prompting Implementation

Refer to plan.md

## Goal
Implement the session_prompt method to connect user prompts to Claude client with session context.

## Tasks

### 1. Session Prompt Method (`lib/src/agent.rs`)

```rust
use agent_client_protocol::{
    PromptRequest, PromptResponse, StopReason, MessageChunk as AcpMessageChunk,
    SessionUpdateNotification, Role,
};

impl Agent for ClaudeAgent {
    async fn session_prompt(&self, request: PromptRequest) -> crate::Result<PromptResponse> {
        self.log_request("session_prompt", &request);
        
        let session_id = uuid::Uuid::parse_str(&request.session_id)
            .map_err(|_| crate::AgentError::Session("Invalid session ID format".to_string()))?;
        
        // Get session context
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
        
        // Query Claude with session context
        let response_content = self.claude_client.query_with_context(
            &request.prompt,
            &session.into(), // Convert to SessionContext
        ).await?;
        
        // Add assistant response to session
        let assistant_message = crate::session::Message {
            role: crate::session::MessageRole::Assistant,
            content: response_content.clone(),
            timestamp: std::time::SystemTime::now(),
        };
        
        self.session_manager.update_session(&session_id, |session| {
            session.add_message(assistant_message);
        })?;
        
        let response = PromptResponse {
            stop_reason: StopReason::EndTurn,
            session_id: request.session_id,
        };
        
        self.log_response("session_prompt", &response);
        Ok(response)
    }
}
```

### 2. Session Context Conversion

```rust
// In lib/src/claude.rs - add conversion from Session to SessionContext

impl From<crate::session::Session> for SessionContext {
    fn from(session: crate::session::Session) -> Self {
        Self {
            session_id: session.id.to_string(),
            messages: session.context,
            created_at: session.created_at,
        }
    }
}

impl From<&crate::session::Session> for SessionContext {
    fn from(session: &crate::session::Session) -> Self {
        Self {
            session_id: session.id.to_string(),
            messages: session.context.clone(),
            created_at: session.created_at,
        }
    }
}
```

### 3. Basic Claude Client Implementation

```rust
// In lib/src/claude.rs - implement non-streaming version first

impl ClaudeClient {
    pub async fn query_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> crate::Result<String> {
        // For now, implement a basic version that just sends the prompt
        // In a real implementation, this would:
        // 1. Build conversation history from context.messages
        // 2. Send to Claude API with full conversation
        // 3. Return the response
        
        tracing::info!("Querying Claude with prompt: {} chars", prompt.len());
        tracing::debug!("Session context has {} messages", context.messages.len());
        
        // Placeholder implementation - replace with real Claude SDK calls
        let response = format!("Response to: {}", prompt);
        
        Ok(response)
    }
    
    pub async fn query(&self, prompt: &str, session_id: &str) -> crate::Result<String> {
        // Simple version without context
        tracing::info!("Simple query for session: {}", session_id);
        
        let response = format!("Simple response to: {}", prompt);
        Ok(response)
    }
}
```

### 4. Error Handling for Prompts

```rust
impl ClaudeAgent {
    async fn validate_prompt_request(&self, request: &PromptRequest) -> crate::Result<()> {
        // Validate session ID format
        uuid::Uuid::parse_str(&request.session_id)
            .map_err(|_| crate::AgentError::Session("Invalid session ID format".to_string()))?;
        
        // Check if prompt is too long (example limit)
        if request.prompt.len() > 100_000 {
            return Err(crate::AgentError::Protocol("Prompt too long".to_string()));
        }
        
        // Check if prompt is empty
        if request.prompt.trim().is_empty() {
            return Err(crate::AgentError::Protocol("Empty prompt".to_string()));
        }
        
        Ok(())
    }
}

// Update session_prompt to use validation
impl Agent for ClaudeAgent {
    async fn session_prompt(&self, request: PromptRequest) -> crate::Result<PromptResponse> {
        self.log_request("session_prompt", &request);
        
        // Validate request
        self.validate_prompt_request(&request).await?;
        
        // ... rest of implementation
    }
}
```

### 5. Response Formatting

```rust
impl ClaudeAgent {
    fn format_response_content(&self, content: &str) -> String {
        // Apply any formatting rules
        // Remove excessive whitespace, handle special characters, etc.
        content.trim().to_string()
    }
    
    fn determine_stop_reason(&self, content: &str) -> StopReason {
        // Logic to determine why the response ended
        // For now, always return EndTurn
        StopReason::EndTurn
    }
}
```

### 6. Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_prompt_flow() {
        let agent = create_test_agent();
        
        // Create session
        let new_session_request = SessionNewRequest {
            client_capabilities: None,
        };
        let new_session_response = agent.session_new(new_session_request).await.unwrap();
        
        // Send prompt
        let prompt_request = PromptRequest {
            session_id: new_session_response.session_id.clone(),
            prompt: "Hello, how are you?".to_string(),
        };
        
        let prompt_response = agent.session_prompt(prompt_request).await.unwrap();
        
        assert_eq!(prompt_response.session_id, new_session_response.session_id);
        assert_eq!(prompt_response.stop_reason, StopReason::EndTurn);
        
        // Verify session was updated
        let session_id = uuid::Uuid::parse_str(&new_session_response.session_id).unwrap();
        let session = agent.session_manager.get_session(&session_id).unwrap().unwrap();
        
        // Should have user message and assistant response
        assert_eq!(session.context.len(), 2);
    }
    
    #[tokio::test]
    async fn test_prompt_validation() {
        let agent = create_test_agent();
        
        // Test invalid session ID
        let prompt_request = PromptRequest {
            session_id: "invalid-uuid".to_string(),
            prompt: "Hello".to_string(),
        };
        
        let result = agent.session_prompt(prompt_request).await;
        assert!(result.is_err());
        
        // Test empty prompt
        let session_response = agent.session_new(SessionNewRequest {
            client_capabilities: None,
        }).await.unwrap();
        
        let prompt_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: "   ".to_string(), // Only whitespace
        };
        
        let result = agent.session_prompt(prompt_request).await;
        assert!(result.is_err());
    }
}
```

## Files Modified
- `lib/src/agent.rs` - Add session_prompt implementation
- `lib/src/claude.rs` - Add context conversion and basic query implementation
- Add integration tests to `lib/src/agent.rs`

## Acceptance Criteria
- session_prompt method processes user input correctly
- Session context is maintained across prompts
- User and assistant messages are stored in session
- Request validation prevents invalid inputs
- Basic Claude client integration works (even with placeholder)
- Integration tests pass showing full prompt flow
- Error handling covers edge cases
- `cargo build` and `cargo test` succeed