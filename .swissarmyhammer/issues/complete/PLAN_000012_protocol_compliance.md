# Protocol Compliance Verification

Refer to plan.md

## Goal
Ensure full ACP specification compliance by implementing any missing methods and comprehensive protocol testing.

## Tasks

### 1. ACP Specification Review

Review the agent-client-protocol specification to ensure all required methods are implemented:

- ✓ `initialize` - Protocol version negotiation
- ✓ `authenticate` - Authentication handling  
- ✓ `session_new` - Session creation
- ✓ `session_prompt` - Prompt handling with streaming
- ⚠ `tool_permission_grant` - Tool permission granting (stub implementation)
- ❓ Additional methods that may be required

### 2. Complete Tool Permission Implementation (`lib/src/agent.rs`)

```rust
use agent_client_protocol::{
    ToolPermissionGrantRequest, ToolPermissionGrantResponse, ToolPermissionDenyRequest, ToolPermissionDenyResponse
};

impl Agent for ClaudeAgent {
    async fn tool_permission_grant(&self, request: ToolPermissionGrantRequest) -> crate::Result<ToolPermissionGrantResponse> {
        tracing::info!("Granting tool permission for call: {}", request.tool_call_id);
        
        // Find and execute the pending tool call
        match self.execute_pending_tool_call(&request.tool_call_id).await {
            Ok(result) => {
                tracing::info!("Tool call {} executed successfully", request.tool_call_id);
                
                // Send tool result as session update
                self.send_tool_result_update(&request.session_id, result).await?;
                
                Ok(ToolPermissionGrantResponse {
                    success: true,
                    error_message: None,
                })
            }
            Err(e) => {
                tracing::error!("Tool call {} failed: {}", request.tool_call_id, e);
                
                Ok(ToolPermissionGrantResponse {
                    success: false,
                    error_message: Some(e.to_string()),
                })
            }
        }
    }
    
    async fn tool_permission_deny(&self, request: ToolPermissionDenyRequest) -> crate::Result<ToolPermissionDenyResponse> {
        tracing::info!("Denying tool permission for call: {}", request.tool_call_id);
        
        // Remove the pending tool call
        self.remove_pending_tool_call(&request.tool_call_id).await?;
        
        // Optionally send notification about denial
        self.send_tool_denial_update(&request.session_id, &request.tool_call_id).await?;
        
        Ok(ToolPermissionDenyResponse {
            success: true,
            error_message: None,
        })
    }
}
```

### 3. Pending Tool Call Management

```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct PendingToolCallManager {
    pending_calls: Arc<RwLock<HashMap<String, (ToolCall, String)>>>, // tool_call_id -> (tool_call, session_id)
}

impl PendingToolCallManager {
    pub fn new() -> Self {
        Self {
            pending_calls: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn add_pending_call(&self, tool_call: ToolCall, session_id: String) -> crate::Result<()> {
        let mut calls = self.pending_calls.write().await;
        calls.insert(tool_call.id.clone(), (tool_call, session_id));
        Ok(())
    }
    
    pub async fn get_pending_call(&self, tool_call_id: &str) -> Option<(ToolCall, String)> {
        let calls = self.pending_calls.read().await;
        calls.get(tool_call_id).cloned()
    }
    
    pub async fn remove_pending_call(&self, tool_call_id: &str) -> Option<(ToolCall, String)> {
        let mut calls = self.pending_calls.write().await;
        calls.remove(tool_call_id)
    }
}

impl ClaudeAgent {
    pub fn new(config: AgentConfig) -> crate::Result<(Self, broadcast::Receiver<SessionUpdateNotification>)> {
        // ... existing initialization ...
        
        let pending_tool_calls = Arc::new(PendingToolCallManager::new());
        
        let agent = Self {
            session_manager: Arc::new(SessionManager::new()),
            claude_client: Arc::new(ClaudeClient::new_with_config(&config.claude)?),
            tool_handler: Arc::new(ToolCallHandler::new_with_terminal_manager(
                config.security.to_tool_permissions()
            )),
            pending_tool_calls,
            // ... rest of fields
        };
        
        Ok((agent, notification_receiver))
    }
    
    async fn execute_pending_tool_call(&self, tool_call_id: &str) -> crate::Result<ToolCallContent> {
        if let Some((tool_call, _session_id)) = self.pending_tool_calls.get_pending_call(tool_call_id).await {
            let result = self.tool_handler.handle_tool_call(tool_call).await?;
            
            match result {
                crate::tools::ToolCallResult::Success(content) => {
                    self.pending_tool_calls.remove_pending_call(tool_call_id).await;
                    Ok(content)
                }
                crate::tools::ToolCallResult::Error(msg) => {
                    self.pending_tool_calls.remove_pending_call(tool_call_id).await;
                    Err(crate::AgentError::ToolExecution(msg))
                }
                crate::tools::ToolCallResult::PermissionRequired(_) => {
                    // This shouldn't happen since we're granting permission
                    Err(crate::AgentError::ToolExecution("Unexpected permission request".to_string()))
                }
            }
        } else {
            Err(crate::AgentError::ToolExecution(format!("Tool call {} not found", tool_call_id)))
        }
    }
    
    async fn remove_pending_tool_call(&self, tool_call_id: &str) -> crate::Result<()> {
        self.pending_tool_calls.remove_pending_call(tool_call_id).await;
        Ok(())
    }
}
```

### 4. Enhanced Notification System

```rust
impl ClaudeAgent {
    async fn send_tool_result_update(&self, session_id: &str, result: ToolCallContent) -> crate::Result<()> {
        let update = SessionUpdateNotification {
            session_id: session_id.to_string(),
            tool_call_result: Some(result),
            message_chunk: None,
        };
        
        self.send_session_update(update).await
    }
    
    async fn send_tool_denial_update(&self, session_id: &str, tool_call_id: &str) -> crate::Result<()> {
        let update = SessionUpdateNotification {
            session_id: session_id.to_string(),
            message_chunk: Some(MessageChunk {
                role: Role::Agent,
                content: vec![ContentBlock::Text {
                    text: format!("Tool call {} was denied by user", tool_call_id),
                }],
            }),
            tool_call_result: None,
        };
        
        self.send_session_update(update).await
    }
    
    async fn send_tool_permission_request(&self, session_id: &str, permission_request: ToolPermissionRequest) -> crate::Result<()> {
        // This would typically be sent to the client via a notification
        // For now, we'll store it as pending and wait for permission grant/deny
        
        let tool_call = ToolCall {
            id: permission_request.tool_call_id.clone(),
            name: permission_request.tool_name.clone(),
            arguments: permission_request.arguments.clone(),
        };
        
        self.pending_tool_calls.add_pending_call(tool_call, session_id.to_string()).await?;
        
        // The actual permission request notification would be sent via transport layer
        // This is typically handled by the server infrastructure
        
        Ok(())
    }
}
```

### 5. Error Code Standardization

```rust
// In lib/src/error.rs - Add ACP-specific error codes

#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("Claude SDK error: {0}")]
    Claude(#[from] claude_sdk_rs::Error),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Tool execution error: {0}")]
    ToolExecution(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Method not found: {0}")]
    MethodNotFound(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AgentError {
    pub fn to_json_rpc_error(&self) -> i32 {
        match self {
            AgentError::Protocol(_) => -32600, // Invalid Request
            AgentError::MethodNotFound(_) => -32601, // Method not found
            AgentError::InvalidRequest(_) => -32602, // Invalid params
            AgentError::Internal(_) => -32603, // Internal error
            AgentError::PermissionDenied(_) => -32000, // Server error
            _ => -32603, // Internal error
        }
    }
}
```

### 6. Protocol Compliance Tests

```rust
#[cfg(test)]
mod protocol_compliance_tests {
    use super::*;
    use agent_client_protocol::*;
    
    async fn create_test_agent() -> (ClaudeAgent, broadcast::Receiver<SessionUpdateNotification>) {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).unwrap()
    }
    
    #[tokio::test]
    async fn test_full_protocol_flow() {
        let (agent, mut notifications) = create_test_agent().await;
        
        // Test initialize
        let init_request = InitializeRequest {
            protocol_version: ProtocolVersion::V1_0_0,
            client_capabilities: Some(ClientCapabilities {
                streaming: Some(true),
                tools: Some(true),
            }),
        };
        
        let init_response = agent.initialize(init_request).await.unwrap();
        assert_eq!(init_response.protocol_version, ProtocolVersion::V1_0_0);
        assert!(init_response.server_capabilities.streaming.unwrap_or(false));
        
        // Test authenticate
        let auth_request = AuthenticateRequest {
            auth_type: "none".to_string(),
            credentials: None,
        };
        
        let auth_response = agent.authenticate(auth_request).await.unwrap();
        assert!(auth_response.success);
        
        // Test session creation
        let session_request = SessionNewRequest {
            client_capabilities: Some(ClientCapabilities {
                streaming: Some(true),
                tools: Some(true),
            }),
        };
        
        let session_response = agent.session_new(session_request).await.unwrap();
        assert!(!session_response.session_id.is_empty());
        
        // Test prompt
        let prompt_request = PromptRequest {
            session_id: session_response.session_id.clone(),
            prompt: "Hello, can you read a file for me?".to_string(),
        };
        
        let prompt_response = agent.session_prompt(prompt_request).await.unwrap();
        assert_eq!(prompt_response.session_id, session_response.session_id);
    }
    
    #[tokio::test]
    async fn test_tool_permission_flow() {
        let (agent, _) = create_test_agent().await;
        
        // Create session
        let session_response = agent.session_new(SessionNewRequest {
            client_capabilities: None,
        }).await.unwrap();
        
        // Simulate tool call that requires permission
        let tool_call = ToolCall {
            id: "test-tool-call".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": "test.txt",
                "content": "Hello, World!"
            }),
        };
        
        // Add to pending calls
        agent.pending_tool_calls.add_pending_call(
            tool_call.clone(), 
            session_response.session_id.clone()
        ).await.unwrap();
        
        // Test permission grant
        let grant_request = ToolPermissionGrantRequest {
            tool_call_id: tool_call.id.clone(),
            session_id: session_response.session_id.clone(),
        };
        
        let grant_response = agent.tool_permission_grant(grant_request).await.unwrap();
        // Response may succeed or fail depending on tool execution
        // but should not error on the protocol level
        
        // Test permission deny
        let tool_call_2 = ToolCall {
            id: "test-tool-call-2".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": "test2.txt",
                "content": "Hello, World 2!"
            }),
        };
        
        agent.pending_tool_calls.add_pending_call(
            tool_call_2.clone(),
            session_response.session_id.clone()
        ).await.unwrap();
        
        let deny_request = ToolPermissionDenyRequest {
            tool_call_id: tool_call_2.id,
            session_id: session_response.session_id,
        };
        
        let deny_response = agent.tool_permission_deny(deny_request).await.unwrap();
        assert!(deny_response.success);
    }
    
    #[tokio::test]
    async fn test_error_handling() {
        let (agent, _) = create_test_agent().await;
        
        // Test invalid session ID
        let invalid_prompt = PromptRequest {
            session_id: "invalid-uuid".to_string(),
            prompt: "Hello".to_string(),
        };
        
        let result = agent.session_prompt(invalid_prompt).await;
        assert!(result.is_err());
        
        // Test unsupported protocol version
        let unsupported_init = InitializeRequest {
            protocol_version: ProtocolVersion::V1_0_0, // Assume this becomes unsupported
            client_capabilities: None,
        };
        
        // This test depends on what versions we actually support
        // let result = agent.initialize(unsupported_init).await;
        // Could be Ok or Err depending on implementation
    }
}
```

### 7. Method Coverage Verification

```rust
// Compile-time check that all Agent trait methods are implemented
fn _compile_time_agent_check() {
    fn assert_agent_impl<T: Agent>() {}
    assert_agent_impl::<ClaudeAgent>();
}

// Runtime verification of capabilities
impl ClaudeAgent {
    pub fn verify_capabilities(&self) -> Vec<String> {
        let mut missing = Vec::new();
        
        // Check required capabilities
        if !self.capabilities.streaming.unwrap_or(false) {
            missing.push("streaming support".to_string());
        }
        
        if self.capabilities.tools.as_ref().map_or(true, |tools| tools.is_empty()) {
            missing.push("tool support".to_string());
        }
        
        // Check required tools
        let required_tools = ["fs_read", "fs_write", "terminal_create"];
        if let Some(ref tools) = self.capabilities.tools {
            for required_tool in &required_tools {
                if !tools.contains(&required_tool.to_string()) {
                    missing.push(format!("tool: {}", required_tool));
                }
            }
        }
        
        missing
    }
}
```

## Files Modified
- `lib/src/agent.rs` - Complete tool permission methods and pending call management
- `lib/src/error.rs` - Add ACP-specific error codes and JSON-RPC error mapping
- Add comprehensive protocol compliance tests

## Acceptance Criteria
- All required Agent trait methods are implemented
- Tool permission granting and denial work correctly
- Pending tool call management is functional
- Error codes comply with JSON-RPC standards
- Full protocol flow tests pass (initialize → authenticate → session_new → session_prompt)
- Tool permission flow tests pass
- Error handling tests cover edge cases
- Capability verification confirms all required features
- `cargo build` and `cargo test` succeed

## Proposed Solution

Based on my analysis of the current codebase, I will implement protocol compliance verification in the following steps:

### 1. Analysis Complete ✓
- Current `ClaudeAgent` implements: `initialize`, `authenticate`, `new_session`, `load_session`, `set_session_mode`, `prompt`, `cancel`, `ext_method`, `ext_notification`
- Missing: Tool permission management system and related methods
- Current error handling needs ACP-specific JSON-RPC error codes

### 2. Implementation Plan
1. **Add Tool Permission Methods**: Implement `tool_permission_grant` and `tool_permission_deny` methods in the Agent trait implementation
2. **Pending Tool Call Management**: Create a system to track tool calls waiting for permission
3. **Enhanced Notification System**: Add support for tool result notifications
4. **Error Code Standardization**: Add JSON-RPC compliant error codes
5. **Comprehensive Testing**: Add protocol compliance tests

### 3. Technical Approach
- Use the detailed implementation examples provided in the issue as a starting point
- Add `PendingToolCallManager` to track tool calls awaiting permission
- Extend the notification system to handle tool call results
- Update error types to include ACP-specific error codes with JSON-RPC mapping
- Write comprehensive tests covering the full protocol flow

### 4. Files to Modify
- `lib/src/agent.rs`: Add tool permission methods and pending call management
- `lib/src/error.rs`: Add ACP error codes and JSON-RPC mapping
- `lib/src/tools.rs`: Integrate with permission system
- Add comprehensive protocol compliance tests


## Implementation Complete ✅

Successfully implemented full ACP protocol compliance with the following completed work:

### 1. Tool Permission Methods ✅
- Added `tool_permission_grant` and `tool_permission_deny` methods to Agent trait implementation
- Both methods handle tool execution and provide proper error handling
- Integrated with existing notification system for real-time updates

### 2. Pending Tool Call Management ✅
- Implemented `PendingToolCallManager` with thread-safe HashMap storage
- Added methods: `add_pending_call`, `get_pending_call`, `remove_pending_call`
- Integrated with ClaudeAgent to track tool calls awaiting permission

### 3. Enhanced Notification System ✅
- Extended `SessionUpdateNotification` to support tool call results
- Added `ToolCallContent` type for tool execution results
- Implemented helper methods: `send_tool_result_update`, `send_tool_denial_update`

### 4. Error Code Standardization ✅
- Added ACP-specific error variants: `PermissionDenied`, `InvalidRequest`, `MethodNotFound`, `Internal`
- Implemented `to_json_rpc_error()` method mapping to standard JSON-RPC error codes
- Added comprehensive error tests

### 5. Protocol Compliance Tests ✅
- `test_full_protocol_flow`: Tests complete initialize → authenticate → session_new → prompt flow
- `test_tool_permission_flow`: Tests tool permission grant/deny cycle
- `test_protocol_error_handling`: Tests error scenarios and edge cases
- `test_pending_tool_call_management`: Tests pending call storage and retrieval
- `test_compile_time_agent_check`: Compile-time verification of Agent trait implementation

### 6. Verification Results ✅
- ✅ All 99 tests pass
- ✅ Cargo build succeeds without warnings
- ✅ All Agent trait methods are implemented
- ✅ Protocol compliance verified through comprehensive testing

## Files Modified
- `lib/src/agent.rs`: Added tool permission methods, pending call management, and enhanced notifications
- `lib/src/error.rs`: Added ACP error codes with JSON-RPC mapping and comprehensive tests

## Technical Implementation Details
- Used Arc<RwLock<HashMap>> for thread-safe pending call storage
- Integrated with existing ToolCallHandler for actual tool execution
- Maintained backward compatibility with existing streaming and session management
- Added proper error propagation and logging throughout