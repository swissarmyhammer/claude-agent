//! Tool call handling infrastructure for Claude Agent
//!
//! This module provides the foundation for parsing, routing, and executing
//! tool requests from LLMs while enforcing security permissions and validations.
//!
//! The agent_client_protocol ToolCall types are for displaying execution status
//! to clients, not for handling incoming requests. This module defines internal
//! types for request handling and converts to protocol types when needed.

use serde_json::Value;

/// Internal representation of a tool request from an LLM
#[derive(Debug, Clone)]
pub struct InternalToolRequest {
    /// Unique identifier for this tool request
    pub id: String,
    /// Name of the tool to execute (e.g., "fs_read", "fs_write")
    pub name: String,
    /// Arguments passed to the tool as JSON
    pub arguments: Value,
}

/// Handles tool request execution with permission management and security validation
#[derive(Debug, Clone)]
pub struct ToolCallHandler {
    permissions: ToolPermissions,
}

/// Configuration for tool permissions and security policies
#[derive(Debug, Clone)]
pub struct ToolPermissions {
    /// Tools that require explicit user permission before execution
    pub require_permission_for: Vec<String>,
    /// Tools that are automatically approved without permission prompts
    pub auto_approved: Vec<String>,
    /// Forbidden path prefixes for file operations
    pub forbidden_paths: Vec<String>,
}

/// Result of a tool request execution attempt
#[derive(Debug, Clone)]
pub enum ToolCallResult {
    /// Tool executed successfully with text response
    Success(String),
    /// Tool requires permission before execution
    PermissionRequired(PermissionRequest),
    /// Tool execution failed with error message
    Error(String),
}

/// Request for permission to execute a tool
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    /// ID of the tool request requiring permission
    pub tool_request_id: String,
    /// Name of the tool requiring permission
    pub tool_name: String,
    /// Human-readable description of what the tool will do
    pub description: String,
    /// Original arguments for the tool request
    pub arguments: Value,
}

impl ToolCallHandler {
    /// Create a new tool call handler with the given permissions
    pub fn new(permissions: ToolPermissions) -> Self {
        Self { permissions }
    }

    /// Handle an incoming tool request, checking permissions and executing if allowed
    pub async fn handle_tool_request(
        &self,
        request: InternalToolRequest,
    ) -> crate::Result<ToolCallResult> {
        tracing::info!("Handling tool request: {}", request.name);

        // Check if permission is required for this tool
        if self.requires_permission(&request.name) {
            let permission_request = self.create_permission_request(&request)?;
            return Ok(ToolCallResult::PermissionRequired(permission_request));
        }

        // Execute the tool request
        match self.execute_tool_request(&request).await {
            Ok(response) => Ok(ToolCallResult::Success(response)),
            Err(e) => Ok(ToolCallResult::Error(e.to_string())),
        }
    }

    /// Check if a tool requires explicit permission
    fn requires_permission(&self, tool_name: &str) -> bool {
        self.permissions
            .require_permission_for
            .contains(&tool_name.to_string())
            && !self
                .permissions
                .auto_approved
                .contains(&tool_name.to_string())
    }
}

impl ToolCallHandler {
    /// Route and execute a tool request based on its name
    async fn execute_tool_request(&self, request: &InternalToolRequest) -> crate::Result<String> {
        match request.name.as_str() {
            "fs_read" => self.handle_fs_read(request).await,
            "fs_write" => self.handle_fs_write(request).await,
            "terminal_create" => self.handle_terminal_create(request).await,
            "terminal_write" => self.handle_terminal_write(request).await,
            _ => Err(crate::AgentError::ToolExecution(format!(
                "Unknown tool: {}",
                request.name
            ))),
        }
    }

    /// Handle file read operations with security validation
    async fn handle_fs_read(&self, request: &InternalToolRequest) -> crate::Result<String> {
        let args = self.parse_tool_args(&request.arguments)?;
        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            crate::AgentError::ToolExecution("Missing 'path' argument".to_string())
        })?;

        tracing::debug!("Reading file: {}", path);

        // Validate path security
        self.validate_file_path(path)?;

        // Read file (placeholder implementation for now)
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to read file {}: {}", path, e))
        })?;

        Ok(content)
    }

    /// Handle file write operations with security validation
    async fn handle_fs_write(&self, request: &InternalToolRequest) -> crate::Result<String> {
        let args = self.parse_tool_args(&request.arguments)?;
        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            crate::AgentError::ToolExecution("Missing 'path' argument".to_string())
        })?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::AgentError::ToolExecution("Missing 'content' argument".to_string())
            })?;

        tracing::debug!("Writing to file: {}", path);

        // Validate path security
        self.validate_file_path(path)?;

        // Write file
        std::fs::write(path, content).map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to write file {}: {}", path, e))
        })?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path
        ))
    }

    /// Handle terminal creation operations
    async fn handle_terminal_create(
        &self,
        _request: &InternalToolRequest,
    ) -> crate::Result<String> {
        // Terminal creation functionality not yet implemented
        Err(crate::AgentError::ToolExecution(
            "Terminal creation is not yet implemented".to_string(),
        ))
    }

    /// Handle terminal write/command execution operations
    async fn handle_terminal_write(&self, _request: &InternalToolRequest) -> crate::Result<String> {
        // Terminal write functionality not yet implemented
        Err(crate::AgentError::ToolExecution(
            "Terminal write is not yet implemented".to_string(),
        ))
    }
}

impl ToolCallHandler {
    /// Validate file path for security violations
    fn validate_file_path(&self, path: &str) -> crate::Result<()> {
        use std::path::Path;

        let path = Path::new(path);

        // Check for directory traversal attempts
        if path.to_string_lossy().contains("..") {
            return Err(crate::AgentError::ToolExecution(
                "Path traversal not allowed".to_string(),
            ));
        }

        // Check for absolute paths outside allowed directories
        if path.is_absolute() {
            let path_str = path.to_string_lossy();

            for prefix in &self.permissions.forbidden_paths {
                if path_str.starts_with(prefix) {
                    return Err(crate::AgentError::ToolExecution(format!(
                        "Access to {} is forbidden",
                        prefix
                    )));
                }
            }
        }

        Ok(())
    }

    /// Parse tool request arguments from JSON to map
    fn parse_tool_args<'a>(
        &self,
        arguments: &'a Value,
    ) -> crate::Result<&'a serde_json::Map<String, Value>> {
        match arguments {
            Value::Object(map) => Ok(map),
            _ => Err(crate::AgentError::ToolExecution(
                "Tool arguments must be an object".to_string(),
            )),
        }
    }
}

impl ToolCallHandler {
    /// Create a permission request for a tool that requires authorization
    fn create_permission_request(
        &self,
        request: &InternalToolRequest,
    ) -> crate::Result<PermissionRequest> {
        let description = match request.name.as_str() {
            "fs_read" => {
                let args = self.parse_tool_args(&request.arguments)?;
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Read file: {}", path)
            }
            "fs_write" => {
                let args = self.parse_tool_args(&request.arguments)?;
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Write to file: {}", path)
            }
            "terminal_create" => "Create terminal session".to_string(),
            "terminal_write" => "Execute terminal command".to_string(),
            _ => format!("Execute tool: {}", request.name),
        };

        Ok(PermissionRequest {
            tool_request_id: request.id.clone(),
            tool_name: request.name.clone(),
            description,
            arguments: request.arguments.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_handler() -> ToolCallHandler {
        let permissions = ToolPermissions {
            require_permission_for: vec!["fs_write".to_string()],
            auto_approved: vec![],
            forbidden_paths: vec![
                "/etc".to_string(),
                "/usr".to_string(),
                "/bin".to_string(),
                "/sys".to_string(),
                "/proc".to_string(),
            ],
        };
        ToolCallHandler::new(permissions)
    }

    #[tokio::test]
    async fn test_fs_read_tool() {
        let handler = create_test_handler();

        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "/safe/path/file.txt"
            }),
        };

        let result = handler.handle_tool_request(request).await;

        // This will fail because the file doesn't exist, but we're testing the flow
        match result {
            Ok(ToolCallResult::Success(_)) => {
                // Success case - file was read
            }
            Ok(ToolCallResult::Error(msg)) => {
                // Expected - file doesn't exist
                assert!(msg.contains("Failed to read file"));
            }
            _ => panic!("Expected success or error result"),
        }
    }

    #[tokio::test]
    async fn test_permission_required() {
        let handler = create_test_handler();

        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": "/safe/path/file.txt",
                "content": "Hello"
            }),
        };

        let result = handler.handle_tool_request(request).await.unwrap();

        match result {
            ToolCallResult::PermissionRequired(perm_req) => {
                assert_eq!(perm_req.tool_request_id, "test-id");
                assert_eq!(perm_req.tool_name, "fs_write");
                assert!(perm_req.description.contains("Write to file"));
            }
            _ => panic!("Expected permission required"),
        }
    }

    #[tokio::test]
    async fn test_path_validation() {
        let handler = create_test_handler();

        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "../../../etc/passwd"
            }),
        };

        let result = handler.handle_tool_request(request).await.unwrap();

        match result {
            ToolCallResult::Error(msg) => {
                // Expected - path traversal should be blocked
                assert!(msg.contains("Path traversal not allowed"));
            }
            _ => panic!("Expected error for dangerous path"),
        }
    }

    #[test]
    fn test_tool_permissions() {
        let permissions = ToolPermissions {
            require_permission_for: vec!["fs_write".to_string(), "terminal_create".to_string()],
            auto_approved: vec!["fs_read".to_string()],
            forbidden_paths: vec!["/etc".to_string(), "/usr".to_string()],
        };

        let handler = ToolCallHandler::new(permissions);

        // fs_write requires permission
        assert!(handler.requires_permission("fs_write"));

        // terminal_create requires permission
        assert!(handler.requires_permission("terminal_create"));

        // fs_read is auto-approved so doesn't require permission
        assert!(!handler.requires_permission("fs_read"));

        // unknown tools don't require permission by default
        assert!(!handler.requires_permission("unknown_tool"));
    }

    #[test]
    fn test_parse_tool_args() {
        let handler = create_test_handler();

        let args = json!({
            "path": "/test/path",
            "content": "test content"
        });

        let parsed = handler.parse_tool_args(&args).unwrap();
        assert_eq!(parsed.get("path").unwrap().as_str().unwrap(), "/test/path");
        assert_eq!(
            parsed.get("content").unwrap().as_str().unwrap(),
            "test content"
        );
    }

    #[test]
    fn test_parse_invalid_args() {
        let handler = create_test_handler();

        // Test non-object arguments
        let args = json!("not an object");
        let result = handler.parse_tool_args(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_file_path_safe() {
        let handler = create_test_handler();

        // These paths should be allowed
        let safe_paths = vec![
            "relative/path/file.txt",
            "./local/file.txt",
            "/home/user/document.txt",
            "/tmp/safe_file.txt",
        ];

        for path in safe_paths {
            assert!(
                handler.validate_file_path(path).is_ok(),
                "Path should be safe: {}",
                path
            );
        }
    }

    #[test]
    fn test_validate_file_path_unsafe() {
        let handler = create_test_handler();

        // These paths should be blocked
        let unsafe_paths = vec![
            "../../../etc/passwd",
            "../../usr/bin/sh",
            "/etc/shadow",
            "/usr/bin/sudo",
            "/bin/bash",
            "/sys/kernel",
            "/proc/version",
        ];

        for path in unsafe_paths {
            assert!(
                handler.validate_file_path(path).is_err(),
                "Path should be blocked: {}",
                path
            );
        }
    }

    #[test]
    fn test_create_permission_request() {
        let handler = create_test_handler();

        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": "/test/file.txt",
                "content": "test"
            }),
        };

        let perm_req = handler.create_permission_request(&request).unwrap();

        assert_eq!(perm_req.tool_request_id, "test-id");
        assert_eq!(perm_req.tool_name, "fs_write");
        assert!(perm_req
            .description
            .contains("Write to file: /test/file.txt"));
        assert_eq!(perm_req.arguments, request.arguments);
    }
}
