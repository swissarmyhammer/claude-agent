# Tool Call Infrastructure

Refer to plan.md

## Goal
Create the foundation for tool call handling including parsing, routing, and permission requests.

## Tasks

### 1. Tool Call Types (`lib/src/tools.rs`)

```rust
use agent_client_protocol::{ToolCall, ToolCallContent, ToolPermissionRequest};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ToolCallHandler {
    permissions: ToolPermissions,
}

#[derive(Debug, Clone)]
pub struct ToolPermissions {
    pub require_permission_for: Vec<String>,
    pub auto_approved: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ToolCallResult {
    Success(ToolCallContent),
    PermissionRequired(ToolPermissionRequest),
    Error(String),
}

impl ToolCallHandler {
    pub fn new(permissions: ToolPermissions) -> Self {
        Self { permissions }
    }
    
    pub async fn handle_tool_call(&self, tool_call: ToolCall) -> crate::Result<ToolCallResult> {
        tracing::info!("Handling tool call: {}", tool_call.name);
        
        // Check if permission is required
        if self.requires_permission(&tool_call.name) {
            let permission_request = self.create_permission_request(&tool_call)?;
            return Ok(ToolCallResult::PermissionRequired(permission_request));
        }
        
        // Execute the tool call
        match self.execute_tool_call(&tool_call).await {
            Ok(content) => Ok(ToolCallResult::Success(content)),
            Err(e) => Ok(ToolCallResult::Error(e.to_string())),
        }
    }
    
    fn requires_permission(&self, tool_name: &str) -> bool {
        self.permissions.require_permission_for.contains(&tool_name.to_string())
            && !self.permissions.auto_approved.contains(&tool_name.to_string())
    }
}
```

### 2. Tool Call Execution Framework

```rust
impl ToolCallHandler {
    async fn execute_tool_call(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        match tool_call.name.as_str() {
            "fs_read" => self.handle_fs_read(tool_call).await,
            "fs_write" => self.handle_fs_write(tool_call).await,
            "terminal_create" => self.handle_terminal_create(tool_call).await,
            "terminal_write" => self.handle_terminal_write(tool_call).await,
            _ => Err(crate::AgentError::ToolExecution(
                format!("Unknown tool: {}", tool_call.name)
            )),
        }
    }
    
    async fn handle_fs_read(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        let args = self.parse_tool_args(&tool_call.arguments)?;
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'path' argument".to_string()))?;
        
        tracing::debug!("Reading file: {}", path);
        
        // Validate path security
        self.validate_file_path(path)?;
        
        // Read file (placeholder implementation)
        let content = format!("Content of file: {}", path);
        
        Ok(ToolCallContent {
            tool_call_id: tool_call.id.clone(),
            content: vec![agent_client_protocol::ContentBlock::Text {
                text: content,
            }],
        })
    }
    
    async fn handle_fs_write(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        let args = self.parse_tool_args(&tool_call.arguments)?;
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'path' argument".to_string()))?;
        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'content' argument".to_string()))?;
        
        tracing::debug!("Writing to file: {}", path);
        
        // Validate path security
        self.validate_file_path(path)?;
        
        // Write file (placeholder implementation)
        let result = format!("Wrote {} bytes to {}", content.len(), path);
        
        Ok(ToolCallContent {
            tool_call_id: tool_call.id.clone(),
            content: vec![agent_client_protocol::ContentBlock::Text {
                text: result,
            }],
        })
    }
    
    async fn handle_terminal_create(&self, _tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        // Placeholder for terminal creation
        todo!("Terminal creation not yet implemented")
    }
    
    async fn handle_terminal_write(&self, _tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        // Placeholder for terminal write
        todo!("Terminal write not yet implemented")
    }
}
```

### 3. Security Validation

```rust
impl ToolCallHandler {
    fn validate_file_path(&self, path: &str) -> crate::Result<()> {
        use std::path::Path;
        
        let path = Path::new(path);
        
        // Check for directory traversal attempts
        if path.to_string_lossy().contains("..") {
            return Err(crate::AgentError::ToolExecution(
                "Path traversal not allowed".to_string()
            ));
        }
        
        // Check for absolute paths outside allowed directories
        if path.is_absolute() {
            let forbidden_prefixes = ["/etc", "/usr", "/bin", "/sys", "/proc"];
            let path_str = path.to_string_lossy();
            
            for prefix in &forbidden_prefixes {
                if path_str.starts_with(prefix) {
                    return Err(crate::AgentError::ToolExecution(
                        format!("Access to {} is forbidden", prefix)
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    fn parse_tool_args(&self, arguments: &Value) -> crate::Result<HashMap<String, Value>> {
        match arguments {
            Value::Object(map) => Ok(map.clone()),
            _ => Err(crate::AgentError::ToolExecution(
                "Tool arguments must be an object".to_string()
            )),
        }
    }
}
```

### 4. Permission Request Creation

```rust
impl ToolCallHandler {
    fn create_permission_request(&self, tool_call: &ToolCall) -> crate::Result<ToolPermissionRequest> {
        let description = match tool_call.name.as_str() {
            "fs_read" => {
                let args = self.parse_tool_args(&tool_call.arguments)?;
                let path = args.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Read file: {}", path)
            }
            "fs_write" => {
                let args = self.parse_tool_args(&tool_call.arguments)?;
                let path = args.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Write to file: {}", path)
            }
            "terminal_create" => "Create terminal session".to_string(),
            "terminal_write" => "Execute terminal command".to_string(),
            _ => format!("Execute tool: {}", tool_call.name),
        };
        
        Ok(ToolPermissionRequest {
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            description,
            arguments: tool_call.arguments.clone(),
        })
    }
}
```

### 5. Configuration Integration

```rust
// In lib/src/config.rs - extend SecurityConfig

impl crate::config::SecurityConfig {
    pub fn to_tool_permissions(&self) -> ToolPermissions {
        ToolPermissions {
            require_permission_for: self.require_permission_for.clone(),
            auto_approved: vec![], // Can be extended later
        }
    }
}

// In lib/src/agent.rs - integrate tool handler

impl ClaudeAgent {
    pub fn new(config: AgentConfig) -> crate::Result<Self> {
        let session_manager = Arc::new(SessionManager::new());
        let claude_client = Arc::new(ClaudeClient::new_with_config(&config.claude)?);
        let tool_handler = Arc::new(crate::tools::ToolCallHandler::new(
            config.security.to_tool_permissions()
        ));
        
        // ... rest of implementation
    }
}
```

### 6. Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    fn create_test_handler() -> ToolCallHandler {
        let permissions = ToolPermissions {
            require_permission_for: vec!["fs_write".to_string()],
            auto_approved: vec![],
        };
        ToolCallHandler::new(permissions)
    }
    
    #[tokio::test]
    async fn test_fs_read_tool() {
        let handler = create_test_handler();
        
        let tool_call = ToolCall {
            id: "test-id".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "/safe/path/file.txt"
            }),
        };
        
        let result = handler.handle_tool_call(tool_call).await.unwrap();
        
        match result {
            ToolCallResult::Success(content) => {
                assert_eq!(content.tool_call_id, "test-id");
            }
            _ => panic!("Expected success result"),
        }
    }
    
    #[tokio::test]
    async fn test_permission_required() {
        let handler = create_test_handler();
        
        let tool_call = ToolCall {
            id: "test-id".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": "/safe/path/file.txt",
                "content": "Hello"
            }),
        };
        
        let result = handler.handle_tool_call(tool_call).await.unwrap();
        
        match result {
            ToolCallResult::PermissionRequired(_) => {
                // Expected
            }
            _ => panic!("Expected permission required"),
        }
    }
    
    #[tokio::test]
    async fn test_path_validation() {
        let handler = create_test_handler();
        
        let tool_call = ToolCall {
            id: "test-id".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "../../../etc/passwd"
            }),
        };
        
        let result = handler.handle_tool_call(tool_call).await.unwrap();
        
        match result {
            ToolCallResult::Error(_) => {
                // Expected - path traversal should be blocked
            }
            _ => panic!("Expected error for dangerous path"),
        }
    }
}
```

## Files Created
- `lib/src/tools.rs` - Tool call handling infrastructure
- Update `lib/src/lib.rs` to export tools module
- Update `lib/src/config.rs` to add tool permissions conversion
- Update `lib/src/agent.rs` to integrate tool handler

## Acceptance Criteria
- Tool calls can be parsed and routed to appropriate handlers
- Permission system works for restricted tools
- File path security validation prevents dangerous operations
- fs_read and fs_write tools have basic implementations
- Error handling covers invalid tool calls and arguments
- Unit tests pass for all tool call scenarios
- `cargo build` and `cargo test` succeed