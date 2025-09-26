# Basic Agent Trait Implementation

Refer to plan.md

## Goal
Implement the foundational methods of the agent-client-protocol Agent trait.

## Tasks

### 1. Agent Structure (`lib/src/agent.rs`)

```rust
use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, InitializeRequest, InitializeResponse,
    SessionNewRequest, SessionNewResponse, ProtocolVersion, ServerCapabilities,
};
use std::sync::Arc;
use crate::{session::SessionManager, claude::ClaudeClient, config::AgentConfig};

pub struct ClaudeAgent {
    session_manager: Arc<SessionManager>,
    claude_client: Arc<ClaudeClient>,
    config: AgentConfig,
    capabilities: ServerCapabilities,
}

impl ClaudeAgent {
    pub fn new(config: AgentConfig) -> crate::Result<Self> {
        let session_manager = Arc::new(SessionManager::new());
        let claude_client = Arc::new(ClaudeClient::new_with_config(&config.claude)?);
        
        let capabilities = ServerCapabilities {
            streaming: Some(true),
            tools: Some(vec![
                "fs_read".to_string(),
                "fs_write".to_string(),
                "terminal_create".to_string(),
                "terminal_write".to_string(),
            ]),
        };
        
        Ok(Self {
            session_manager,
            claude_client,
            config,
            capabilities,
        })
    }
}
```

### 2. Initialize Method

```rust
#[async_trait::async_trait]
impl Agent for ClaudeAgent {
    async fn initialize(&self, request: InitializeRequest) -> crate::Result<InitializeResponse> {
        tracing::info!("Initializing agent with protocol version: {:?}", request.protocol_version);
        
        // Validate protocol version compatibility
        match request.protocol_version {
            ProtocolVersion::V1_0_0 => {
                tracing::info!("Protocol version 1.0.0 supported");
            }
            _ => {
                return Err(crate::AgentError::Protocol(
                    format!("Unsupported protocol version: {:?}", request.protocol_version)
                ));
            }
        }
        
        // Log client capabilities for debugging
        if let Some(ref capabilities) = request.client_capabilities {
            tracing::debug!("Client capabilities: {:?}", capabilities);
        }
        
        Ok(InitializeResponse {
            server_capabilities: self.capabilities.clone(),
            protocol_version: ProtocolVersion::V1_0_0,
            server_info: Some(agent_client_protocol::ServerInfo {
                name: "Claude Agent".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
        })
    }
}
```

### 3. Authenticate Method

```rust
impl Agent for ClaudeAgent {
    async fn authenticate(&self, request: AuthenticateRequest) -> crate::Result<AuthenticateResponse> {
        tracing::info!("Authentication requested");
        
        // For now, delegate authentication to Claude Code
        // In the future, this might involve API key validation
        
        match request.auth_type.as_str() {
            "none" => {
                tracing::info!("No authentication required");
                Ok(AuthenticateResponse {
                    success: true,
                    error_message: None,
                })
            }
            "api_key" => {
                // TODO: Implement API key validation with Claude SDK
                tracing::warn!("API key authentication not yet implemented");
                Ok(AuthenticateResponse {
                    success: true, // Temporary - always succeed
                    error_message: None,
                })
            }
            _ => {
                let error_msg = format!("Unsupported auth type: {}", request.auth_type);
                tracing::error!("{}", error_msg);
                Ok(AuthenticateResponse {
                    success: false,
                    error_message: Some(error_msg),
                })
            }
        }
    }
}
```

### 4. Session New Method

```rust
impl Agent for ClaudeAgent {
    async fn session_new(&self, request: SessionNewRequest) -> crate::Result<SessionNewResponse> {
        tracing::info!("Creating new session");
        
        let session_id = self.session_manager.create_session()?;
        
        // Store client capabilities in the session if provided
        if let Some(capabilities) = request.client_capabilities {
            self.session_manager.update_session(&session_id, |session| {
                session.client_capabilities = Some(capabilities);
            })?;
        }
        
        tracing::info!("Created session: {}", session_id);
        
        Ok(SessionNewResponse {
            session_id: session_id.to_string(),
        })
    }
}
```

### 5. Error Handling and Logging

```rust
impl ClaudeAgent {
    fn log_request<T: std::fmt::Debug>(&self, method: &str, request: &T) {
        tracing::debug!("Handling {} request: {:?}", method, request);
    }
    
    fn log_response<T: std::fmt::Debug>(&self, method: &str, response: &T) {
        tracing::debug!("Returning {} response: {:?}", method, response);
    }
}

// Update trait methods to include logging
impl Agent for ClaudeAgent {
    async fn initialize(&self, request: InitializeRequest) -> crate::Result<InitializeResponse> {
        self.log_request("initialize", &request);
        
        // ... existing implementation ...
        
        let response = Ok(InitializeResponse { /* ... */ });
        if let Ok(ref resp) = response {
            self.log_response("initialize", resp);
        }
        response
    }
    
    // Similar logging for other methods
}
```

### 6. Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::*;
    
    fn create_test_agent() -> ClaudeAgent {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).unwrap()
    }
    
    #[tokio::test]
    async fn test_initialize() {
        let agent = create_test_agent();
        
        let request = InitializeRequest {
            protocol_version: ProtocolVersion::V1_0_0,
            client_capabilities: None,
        };
        
        let response = agent.initialize(request).await.unwrap();
        
        assert_eq!(response.protocol_version, ProtocolVersion::V1_0_0);
        assert!(response.server_capabilities.streaming.unwrap_or(false));
    }
    
    #[tokio::test]
    async fn test_authenticate_none() {
        let agent = create_test_agent();
        
        let request = AuthenticateRequest {
            auth_type: "none".to_string(),
            credentials: None,
        };
        
        let response = agent.authenticate(request).await.unwrap();
        assert!(response.success);
    }
    
    #[tokio::test]
    async fn test_session_new() {
        let agent = create_test_agent();
        
        let request = SessionNewRequest {
            client_capabilities: None,
        };
        
        let response = agent.session_new(request).await.unwrap();
        assert!(!response.session_id.is_empty());
    }
}
```

## Files Created
- `lib/src/agent.rs` - Agent trait implementation
- Update `lib/src/lib.rs` to export agent module

## Acceptance Criteria
- Agent can be created with default configuration
- Initialize method handles protocol negotiation
- Authenticate method supports "none" and "api_key" types
- Session creation returns valid session IDs
- All methods include proper logging
- Unit tests pass for all implemented methods
- `cargo build` and `cargo test` succeed