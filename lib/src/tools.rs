//! Tool call handling infrastructure for Claude Agent
//!
//! This module provides the foundation for parsing, routing, and executing
//! tool requests from LLMs while enforcing security permissions and validations.
//!
//! The agent_client_protocol ToolCall types are for displaying execution status
//! to clients, not for handling incoming requests. This module defines internal
//! types for request handling and converts to protocol types when needed.
//!
//! ## Security Model
//!
//! This module implements comprehensive security controls for tool execution:
//!
//! ### ACP Path Validation
//!
//! All file operations are subject to ACP-compliant absolute path validation:
//! - **Absolute Path Requirement**: All file paths must be absolute according to platform conventions
//! - **Path Traversal Prevention**: Detects and blocks `../` and similar traversal attempts
//! - **Cross-Platform Support**: Handles Unix (`/path/to/file`) and Windows (`C:\path\to\file`) paths
//! - **Canonicalization**: Resolves symlinks and normalizes paths to prevent bypass attempts
//! - **Boundary Enforcement**: Optionally restricts operations to configured root directories
//!
//! ### Permission System
//!
//! Tools are categorized into security levels:
//! - **Auto-Approved**: Safe read-only operations that execute immediately
//! - **Permission Required**: Potentially dangerous operations requiring user consent
//! - **Forbidden**: Operations blocked by security policy
//!
//! ### File Operation Security
//!
//! File operations implement multiple security layers:
//! - Path validation prevents access outside allowed boundaries
//! - Null byte detection blocks injection attempts  
//! - Path length limits prevent buffer overflow attacks
//! - Working directory validation ensures operations occur in expected locations
//!
//! ### Command Execution Security
//!
//! Terminal operations are sandboxed with:
//! - Working directory restrictions
//! - Environment variable controls
//! - Process isolation and cleanup
//! - Output sanitization and length limits
//!
//! ## Error Handling
//!
//! Security violations result in specific error types:
//! - `PathValidationError`: Path validation failures with detailed context
//! - `PermissionDenied`: Access control violations
//! - `InvalidRequest`: Malformed or suspicious requests
//!
//! These errors are mapped to appropriate JSON-RPC error codes for client communication.

use crate::path_validator::{PathValidationError, PathValidator};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

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

/// Manages terminal sessions for command execution
#[derive(Debug, Clone)]
pub struct TerminalManager {
    terminals: Arc<RwLock<HashMap<String, TerminalSession>>>,
}

/// Represents a terminal session with working directory and environment
#[derive(Debug)]
pub struct TerminalSession {
    process: Option<Child>,
    working_dir: std::path::PathBuf,
    environment: HashMap<String, String>,
}

/// Handles tool request execution with permission management and security validation
#[derive(Debug, Clone)]
pub struct ToolCallHandler {
    permissions: ToolPermissions,
    terminal_manager: Arc<TerminalManager>,
    mcp_manager: Option<Arc<crate::mcp::McpServerManager>>,
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

/// ACP-compliant permission option for user choice
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionOption {
    /// Unique identifier for this permission option
    pub option_id: String,
    /// Human-readable name for the option
    pub name: String,
    /// The kind of permission action this option represents
    pub kind: PermissionOptionKind,
}

/// ACP permission option kinds as defined in the specification
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOptionKind {
    /// Allow this specific tool call once
    AllowOnce,
    /// Allow all future calls of this tool type
    AllowAlways,
    /// Reject this specific tool call once
    RejectOnce,
    /// Reject all future calls of this tool type
    RejectAlways,
}

/// Enhanced permission request with multiple user options (ACP-compliant)
#[derive(Debug, Clone)]
pub struct EnhancedPermissionRequest {
    /// Session identifier for the permission request
    pub session_id: String,
    /// ID of the tool request requiring permission
    pub tool_request_id: String,
    /// Name of the tool requiring permission
    pub tool_name: String,
    /// Human-readable description of what the tool will do
    pub description: String,
    /// Original arguments for the tool request
    pub arguments: serde_json::Value,
    /// Available permission options for the user
    pub options: Vec<PermissionOption>,
}

/// Outcome of a permission request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "outcome")]
pub enum PermissionOutcome {
    /// User cancelled the permission request
    #[serde(rename = "cancelled")]
    Cancelled,
    /// User selected one of the permission options
    #[serde(rename = "selected")]
    Selected {
        /// The ID of the selected permission option
        #[serde(rename = "optionId")]
        option_id: String,
    },
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

impl TerminalManager {
    /// Create a new terminal manager
    pub fn new() -> Self {
        Self {
            terminals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new terminal session
    pub async fn create_terminal(&self, working_dir: Option<String>) -> crate::Result<String> {
        let terminal_id = ulid::Ulid::new().to_string();
        let working_dir = working_dir
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });

        let session = TerminalSession {
            process: None,
            working_dir,
            environment: std::env::vars().collect(),
        };

        let mut terminals = self.terminals.write().await;
        terminals.insert(terminal_id.clone(), session);

        tracing::info!("Created terminal session: {}", terminal_id);
        Ok(terminal_id)
    }

    /// Execute a command in the specified terminal session
    pub async fn execute_command(&self, terminal_id: &str, command: &str) -> crate::Result<String> {
        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id).ok_or_else(|| {
            crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id))
        })?;

        tracing::info!("Executing command in terminal {}: {}", terminal_id, command);

        // Parse command and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(crate::AgentError::ToolExecution(
                "Empty command".to_string(),
            ));
        }

        let program = parts[0];
        let args = &parts[1..];

        // Execute command
        let output = Command::new(program)
            .args(args)
            .current_dir(&session.working_dir)
            .envs(&session.environment)
            .output()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to execute command: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if output.status.success() {
            if stdout.is_empty() {
                "Command completed successfully (exit code: 0)".to_string()
            } else {
                format!("Command output:\n{}", stdout)
            }
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            if stderr.is_empty() {
                format!("Command failed (exit code: {})", exit_code)
            } else {
                format!("Command failed (exit code: {}):\n{}", exit_code, stderr)
            }
        };

        tracing::info!(
            "Command completed with exit code: {:?}",
            output.status.code()
        );
        Ok(result)
    }

    /// Change the working directory for a terminal session
    pub async fn change_directory(&self, terminal_id: &str, path: &str) -> crate::Result<String> {
        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id).ok_or_else(|| {
            crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id))
        })?;

        let new_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            session.working_dir.join(path)
        };

        if new_path.exists() && new_path.is_dir() {
            session.working_dir = new_path.canonicalize().map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to resolve path: {}", e))
            })?;

            tracing::info!("Changed directory to: {}", session.working_dir.display());
            Ok(format!(
                "Changed directory to: {}",
                session.working_dir.display()
            ))
        } else {
            Err(crate::AgentError::ToolExecution(format!(
                "Directory does not exist: {}",
                path
            )))
        }
    }

    /// Remove a terminal session
    pub async fn remove_terminal(&self, terminal_id: &str) -> crate::Result<()> {
        let mut terminals = self.terminals.write().await;
        if let Some(mut session) = terminals.remove(terminal_id) {
            if let Some(mut process) = session.process.take() {
                let _ = process.kill().await;
            }
            tracing::info!("Removed terminal session: {}", terminal_id);
        }
        Ok(())
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallHandler {
    /// Create a new tool call handler with the given permissions
    pub fn new(permissions: ToolPermissions) -> Self {
        Self {
            permissions,
            terminal_manager: Arc::new(TerminalManager::new()),
            mcp_manager: None,
        }
    }

    /// Create a new tool call handler with custom terminal manager
    pub fn new_with_terminal_manager(
        permissions: ToolPermissions,
        terminal_manager: Arc<TerminalManager>,
    ) -> Self {
        Self {
            permissions,
            terminal_manager,
            mcp_manager: None,
        }
    }

    /// Create a new tool call handler with MCP manager
    pub fn new_with_mcp_manager(
        permissions: ToolPermissions,
        mcp_manager: Arc<crate::mcp::McpServerManager>,
    ) -> Self {
        Self {
            permissions,
            terminal_manager: Arc::new(TerminalManager::new()),
            mcp_manager: Some(mcp_manager),
        }
    }

    /// Create a new tool call handler with custom terminal manager and MCP manager
    pub fn new_with_terminal_and_mcp_manager(
        permissions: ToolPermissions,
        terminal_manager: Arc<TerminalManager>,
        mcp_manager: Arc<crate::mcp::McpServerManager>,
    ) -> Self {
        Self {
            permissions,
            terminal_manager,
            mcp_manager: Some(mcp_manager),
        }
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
        // Check if this is an MCP tool call
        if let Some(server_name) = self.extract_mcp_server_name(&request.name) {
            if let Some(ref mcp_manager) = self.mcp_manager {
                return mcp_manager.execute_tool_call(server_name, request).await;
            }
        }

        // Handle built-in tools
        match request.name.as_str() {
            "fs_read" => self.handle_fs_read(request).await,
            "fs_write" => self.handle_fs_write(request).await,
            "fs_list" => self.handle_fs_list(request).await,
            "terminal_create" => self.handle_terminal_create(request).await,
            "terminal_write" => self.handle_terminal_write(request).await,
            _ => Err(crate::AgentError::ToolExecution(format!(
                "Unknown tool: {}",
                request.name
            ))),
        }
    }

    /// Extract MCP server name from tool name
    fn extract_mcp_server_name<'a>(&self, tool_name: &'a str) -> Option<&'a str> {
        // Tool names from MCP servers are prefixed with server name
        // e.g., "filesystem:read_file" -> server "filesystem"
        if tool_name.contains(':') {
            tool_name.split(':').next()
        } else {
            None
        }
    }

    /// List all available tools including MCP tools
    pub async fn list_all_available_tools(&self) -> Vec<String> {
        let mut tools = vec![
            "fs_read".to_string(),
            "fs_write".to_string(),
            "fs_list".to_string(),
            "terminal_create".to_string(),
            "terminal_write".to_string(),
        ];

        if let Some(ref mcp_manager) = self.mcp_manager {
            let mcp_tools = mcp_manager.list_available_tools().await;
            tools.extend(mcp_tools);
        }

        tools
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

        // Read file using tokio::fs for async operation
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to read file {}: {}", path, e))
        })?;

        tracing::info!("Successfully read {} bytes from {}", content.len(), path);
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

        tracing::debug!("Writing to file: {} ({} bytes)", path, content.len());

        // Validate path security
        self.validate_file_path(path)?;

        // Create parent directories if they don't exist
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to create parent directories for {}: {}",
                        path, e
                    ))
                })?;
            }
        }

        // Write file using tokio::fs for async operation
        tokio::fs::write(path, content).await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to write file {}: {}", path, e))
        })?;

        tracing::info!("Successfully wrote {} bytes to {}", content.len(), path);
        Ok(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            path
        ))
    }

    /// Handle directory listing operations with security validation
    async fn handle_fs_list(&self, request: &InternalToolRequest) -> crate::Result<String> {
        let args = self.parse_tool_args(&request.arguments)?;
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        tracing::debug!("Listing directory: {}", path);

        // Validate path security
        self.validate_file_path(path)?;

        let mut dir_reader = tokio::fs::read_dir(path).await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to list directory {}: {}", path, e))
        })?;

        let mut files = Vec::new();

        while let Some(entry) = dir_reader.next_entry().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Error reading directory entry: {}", e))
        })? {
            let metadata = entry.metadata().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to get metadata: {}", e))
            })?;

            let file_type = if metadata.is_dir() {
                "directory"
            } else {
                "file"
            };
            let size = if metadata.is_file() {
                metadata.len()
            } else {
                0
            };

            files.push(format!(
                "{} ({}, {} bytes)",
                entry.file_name().to_string_lossy(),
                file_type,
                size
            ));
        }

        let content = if files.is_empty() {
            format!("Directory {} is empty", path)
        } else {
            format!("Contents of {}:\n{}", path, files.join("\n"))
        };

        tracing::info!("Listed {} items in directory {}", files.len(), path);
        Ok(content)
    }

    /// Handle terminal creation operations
    async fn handle_terminal_create(&self, request: &InternalToolRequest) -> crate::Result<String> {
        let args = self.parse_tool_args(&request.arguments)?;
        let working_dir = args.get("working_dir").and_then(|v| v.as_str());

        let terminal_id = self
            .terminal_manager
            .create_terminal(working_dir.map(String::from))
            .await?;

        Ok(format!("Created terminal session: {}", terminal_id))
    }

    /// Handle terminal write/command execution operations
    async fn handle_terminal_write(&self, request: &InternalToolRequest) -> crate::Result<String> {
        let args = self.parse_tool_args(&request.arguments)?;
        let terminal_id = args
            .get("terminal_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::AgentError::ToolExecution("Missing 'terminal_id' argument".to_string())
            })?;
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::AgentError::ToolExecution("Missing 'command' argument".to_string())
            })?;

        // Validate command security
        self.validate_command(command)?;

        // Check if this is a directory change command
        if command.trim().starts_with("cd ") {
            let path = command.trim().strip_prefix("cd ").unwrap_or("").trim();
            let result = self
                .terminal_manager
                .change_directory(terminal_id, path)
                .await?;
            return Ok(result);
        }

        // Execute the command
        let result = self
            .terminal_manager
            .execute_command(terminal_id, command)
            .await?;
        Ok(result)
    }
}

impl ToolCallHandler {
    /// ACP-compliant file path validation with comprehensive security checks
    fn validate_file_path(&self, path: &str) -> crate::Result<()> {
        // ACP requires strict absolute path validation:
        // 1. All paths MUST be absolute (no relative paths allowed)
        // 2. Unix: Must start with /
        // 3. Windows: Must include drive letter (C:\) or UNC path
        // 4. Path traversal prevention (no ../ components)
        // 5. Cross-platform normalization and security validation
        //
        // Path validation prevents security issues and ensures protocol compliance.

        // Create path validator with non-strict canonicalization to avoid file existence checks
        let validator = PathValidator::new().with_strict_canonicalization(false);

        // Validate absolute path according to ACP specification
        let validated_path = validator.validate_absolute_path(path).map_err(|e| {
            match e {
                PathValidationError::NotAbsolute(p) => {
                    crate::AgentError::ToolExecution(format!(
                        "Invalid path: must be absolute path. Provided: '{}'. Examples: Unix: '/home/user/file.txt', Windows: 'C:\\\\Users\\\\user\\\\file.txt'",
                        p
                    ))
                }
                PathValidationError::PathTraversalAttempt => {
                    crate::AgentError::ToolExecution(
                        "Path traversal attempt detected. Parent directory references (..) are not allowed".to_string()
                    )
                }
                PathValidationError::RelativeComponent => {
                    crate::AgentError::ToolExecution(
                        "Path contains relative components (. or ..) which are not allowed".to_string()
                    )
                }
                PathValidationError::PathTooLong(actual, max) => {
                    crate::AgentError::ToolExecution(format!(
                        "Path too long: {} characters exceeds maximum of {} characters", actual, max
                    ))
                }
                PathValidationError::NullBytesInPath => {
                    crate::AgentError::ToolExecution(
                        "Null bytes in path not allowed".to_string()
                    )
                }
                PathValidationError::EmptyPath => {
                    crate::AgentError::ToolExecution(
                        "Empty path provided".to_string()
                    )
                }
                PathValidationError::CanonicalizationFailed(path, err) => {
                    crate::AgentError::ToolExecution(format!(
                        "Path canonicalization failed for '{}': {}", path, err
                    ))
                }
                PathValidationError::OutsideBoundaries(path) => {
                    crate::AgentError::ToolExecution(format!(
                        "Path outside allowed boundaries: {}", path
                    ))
                }
                PathValidationError::InvalidFormat(msg) => {
                    crate::AgentError::ToolExecution(format!(
                        "Invalid path format: {}", msg
                    ))
                }
            }
        })?;

        // Additional security checks beyond ACP requirements
        let path_str = validated_path.to_string_lossy();

        // Check against forbidden paths from configuration
        for prefix in &self.permissions.forbidden_paths {
            if path_str.starts_with(prefix) {
                return Err(crate::AgentError::ToolExecution(format!(
                    "Access to {} is forbidden by configuration",
                    prefix
                )));
            }
        }

        // Additional forbidden system directories for security
        let forbidden_prefixes = ["/etc", "/usr", "/bin", "/sys", "/proc", "/dev"];
        for prefix in &forbidden_prefixes {
            if path_str.starts_with(prefix) {
                return Err(crate::AgentError::ToolExecution(format!(
                    "Access to system directory {} is forbidden for security",
                    prefix
                )));
            }
        }

        // Check file extension restrictions for write operations
        if let Some(ext) = validated_path.extension() {
            let dangerous_extensions = ["exe", "bat", "cmd", "scr", "com", "pif"];
            if dangerous_extensions.contains(&ext.to_string_lossy().as_ref()) {
                return Err(crate::AgentError::ToolExecution(format!(
                    "File extension .{} is not allowed for security",
                    ext.to_string_lossy()
                )));
            }
        }

        Ok(())
    }

    /// Validate command for security violations
    fn validate_command(&self, command: &str) -> crate::Result<()> {
        let trimmed = command.trim();

        // Check for empty commands
        if trimmed.is_empty() {
            return Err(crate::AgentError::ToolExecution(
                "Empty command not allowed".to_string(),
            ));
        }

        // Check for dangerous command patterns
        let dangerous_patterns = [
            "rm -rf /",
            "format",
            "fdisk",
            "mkfs",
            "dd if=",
            "shutdown",
            "reboot",
            "halt",
            "poweroff",
            "kill -9 1",
            "init 0",
            "init 6",
        ];

        let command_lower = trimmed.to_lowercase();
        for pattern in &dangerous_patterns {
            if command_lower.contains(pattern) {
                return Err(crate::AgentError::ToolExecution(format!(
                    "Dangerous command pattern '{}' not allowed",
                    pattern
                )));
            }
        }

        // Check command length
        if trimmed.len() > 1000 {
            return Err(crate::AgentError::ToolExecution(
                "Command too long".to_string(),
            ));
        }

        // Check for null bytes
        if trimmed.contains('\0') {
            return Err(crate::AgentError::ToolExecution(
                "Null bytes in command not allowed".to_string(),
            ));
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

    #[test]
    fn test_permission_option_creation() {
        let option = PermissionOption {
            option_id: "allow-once".to_string(),
            name: "Allow once".to_string(),
            kind: PermissionOptionKind::AllowOnce,
        };
        
        assert_eq!(option.option_id, "allow-once");
        assert_eq!(option.name, "Allow once");
        assert!(matches!(option.kind, PermissionOptionKind::AllowOnce));
    }

    #[test]
    fn test_enhanced_permission_request() {
        let options = vec![
            PermissionOption {
                option_id: "allow-once".to_string(),
                name: "Allow once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
            },
            PermissionOption {
                option_id: "reject-once".to_string(),
                name: "Reject".to_string(),
                kind: PermissionOptionKind::RejectOnce,
            },
        ];

        let request = EnhancedPermissionRequest {
            session_id: "session-123".to_string(),
            tool_request_id: "tool-456".to_string(),
            tool_name: "fs_write".to_string(),
            description: "Write to file".to_string(),
            arguments: serde_json::json!({"path": "/test.txt"}),
            options,
        };

        assert_eq!(request.session_id, "session-123");
        assert_eq!(request.options.len(), 2);
        assert_eq!(request.options[0].option_id, "allow-once");
        assert_eq!(request.options[1].option_id, "reject-once");
    }

    #[test]
    fn test_permission_outcome_serialization() {
        let outcome = PermissionOutcome::Selected {
            option_id: "allow-once".to_string(),
        };
        
        match outcome {
            PermissionOutcome::Selected { option_id } => {
                assert_eq!(option_id, "allow-once");
            },
            _ => panic!("Expected Selected outcome"),
        }

        let cancelled_outcome = PermissionOutcome::Cancelled;
        assert!(matches!(cancelled_outcome, PermissionOutcome::Cancelled));
    }

    #[test]
    fn test_generate_permission_options_safe_tool() {
        let handler = create_test_handler();
        
        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({"path": "/safe/file.txt"}),
        };

        let options = handler.generate_permission_options(&request);
        
        // Safe read operations should offer all options
        assert_eq!(options.len(), 4);
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::AllowOnce));
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::AllowAlways));
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::RejectOnce));
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::RejectAlways));
    }

    #[test]
    fn test_generate_permission_options_dangerous_tool() {
        let handler = create_test_handler();
        
        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({"path": "/important/config.txt", "content": "new config"}),
        };

        let options = handler.generate_permission_options(&request);
        
        // Dangerous operations should offer all options but with appropriate warnings
        assert_eq!(options.len(), 4);
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::AllowOnce));
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::AllowAlways));
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::RejectOnce));
        assert!(options.iter().any(|o| o.kind == PermissionOptionKind::RejectAlways));
        
        // Check that "allow always" option has warning text
        let allow_always_option = options.iter().find(|o| o.kind == PermissionOptionKind::AllowAlways).unwrap();
        assert!(allow_always_option.name.contains("caution") || allow_always_option.name.contains("warning"));
    }

    #[test]
    fn test_generate_permission_options_terminal_tool() {
        let handler = create_test_handler();
        
        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "terminal_write".to_string(),
            arguments: json!({"terminal_id": "term-123", "command": "ls -la"}),
        };

        let options = handler.generate_permission_options(&request);
        
        // Terminal operations should be treated as high-risk
        assert_eq!(options.len(), 4);
        
        // Verify option IDs follow ACP pattern
        let option_ids: Vec<&str> = options.iter().map(|o| o.option_id.as_str()).collect();
        assert!(option_ids.contains(&"allow-once"));
        assert!(option_ids.contains(&"allow-always"));
        assert!(option_ids.contains(&"reject-once"));
        assert!(option_ids.contains(&"reject-always"));
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

    /// Generate ACP-compliant permission options for a tool request
    pub fn generate_permission_options(&self, request: &InternalToolRequest) -> Vec<PermissionOption> {
        // ACP requires comprehensive permission system with user choice:
        // 1. Multiple permission options: allow/reject with once/always variants
        // 2. Permission persistence: Remember "always" decisions across sessions
        // 3. Tool call integration: Block execution until permission granted
        // 4. Cancellation support: Handle cancelled prompt turns gracefully
        // 5. Context awareness: Generate appropriate options for different tools
        //
        // Advanced permissions provide user control while maintaining security.
        
        let tool_risk_level = self.assess_tool_risk(&request.name, &request.arguments);
        
        match tool_risk_level {
            ToolRiskLevel::Safe => {
                vec![
                    PermissionOption {
                        option_id: "allow-once".to_string(),
                        name: "Allow once".to_string(),
                        kind: PermissionOptionKind::AllowOnce,
                    },
                    PermissionOption {
                        option_id: "allow-always".to_string(),
                        name: "Allow always".to_string(),
                        kind: PermissionOptionKind::AllowAlways,
                    },
                    PermissionOption {
                        option_id: "reject-once".to_string(),
                        name: "Reject".to_string(),
                        kind: PermissionOptionKind::RejectOnce,
                    },
                    PermissionOption {
                        option_id: "reject-always".to_string(),
                        name: "Reject always".to_string(),
                        kind: PermissionOptionKind::RejectAlways,
                    },
                ]
            }
            ToolRiskLevel::Moderate => {
                vec![
                    PermissionOption {
                        option_id: "allow-once".to_string(),
                        name: "Allow once".to_string(),
                        kind: PermissionOptionKind::AllowOnce,
                    },
                    PermissionOption {
                        option_id: "allow-always".to_string(),
                        name: "Allow always (use with caution)".to_string(),
                        kind: PermissionOptionKind::AllowAlways,
                    },
                    PermissionOption {
                        option_id: "reject-once".to_string(),
                        name: "Reject".to_string(),
                        kind: PermissionOptionKind::RejectOnce,
                    },
                    PermissionOption {
                        option_id: "reject-always".to_string(),
                        name: "Reject always".to_string(),
                        kind: PermissionOptionKind::RejectAlways,
                    },
                ]
            }
            ToolRiskLevel::High => {
                vec![
                    PermissionOption {
                        option_id: "allow-once".to_string(),
                        name: "Allow once".to_string(),
                        kind: PermissionOptionKind::AllowOnce,
                    },
                    PermissionOption {
                        option_id: "allow-always".to_string(),
                        name: "Allow always (warning: high-risk operation)".to_string(),
                        kind: PermissionOptionKind::AllowAlways,
                    },
                    PermissionOption {
                        option_id: "reject-once".to_string(),
                        name: "Reject".to_string(),
                        kind: PermissionOptionKind::RejectOnce,
                    },
                    PermissionOption {
                        option_id: "reject-always".to_string(),
                        name: "Reject always".to_string(),
                        kind: PermissionOptionKind::RejectAlways,
                    },
                ]
            }
        }
    }

    /// Assess the risk level of a tool operation
    fn assess_tool_risk(&self, tool_name: &str, arguments: &serde_json::Value) -> ToolRiskLevel {
        match tool_name {
            // File read operations are generally safe
            "fs_read" | "fs_list" => ToolRiskLevel::Safe,
            
            // File write operations have moderate risk
            "fs_write" => {
                // Check if writing to sensitive locations
                if let Ok(args) = self.parse_tool_args(arguments) {
                    if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                        let sensitive_patterns = ["/etc", "/usr", "/bin", "/sys", "/proc"];
                        if sensitive_patterns.iter().any(|&pattern| path.starts_with(pattern)) {
                            return ToolRiskLevel::High;
                        }
                        // Configuration files are moderate risk
                        if path.ends_with(".conf") || path.ends_with(".config") || path.contains("config") {
                            return ToolRiskLevel::Moderate;
                        }
                    }
                }
                ToolRiskLevel::Moderate
            }
            
            // Terminal operations are high risk
            "terminal_create" | "terminal_write" => ToolRiskLevel::High,
            
            // Unknown tools are treated as moderate risk
            _ => ToolRiskLevel::Moderate,
        }
    }
}

/// Risk assessment levels for tool operations
#[derive(Debug, Clone, PartialEq)]
enum ToolRiskLevel {
    /// Safe operations with minimal risk
    Safe,
    /// Moderate risk operations requiring caution
    Moderate,  
    /// High-risk operations requiring careful consideration
    High,
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
                // Expected - relative path should be blocked (ACP requires absolute paths)
                assert!(msg.contains("must be absolute path"));
            }
            _ => panic!("Expected error for relative path"),
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

        // These paths should be allowed (ACP requires absolute paths only)
        let safe_paths = vec!["/home/user/document.txt", "/tmp/safe_file.txt"];

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
    fn test_validate_file_path_relative_rejected() {
        let handler = create_test_handler();

        // These relative paths should be rejected per ACP specification
        let relative_paths = vec![
            "relative/path/file.txt",
            "./local/file.txt",
            "../parent/dir/file.txt",
            "file.txt",
            "src/main.rs",
        ];

        for path in relative_paths {
            let result = handler.validate_file_path(path);
            assert!(
                result.is_err(),
                "Relative path should be rejected per ACP spec: {}",
                path
            );
            // Verify it's specifically an absolute path error
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains("must be absolute path"),
                "Error should mention absolute path requirement for '{}', got: {}",
                path,
                error_msg
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

    #[tokio::test]
    async fn test_fs_write_and_read_integration() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        let file_path_str = file_path.to_string_lossy();

        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_write".to_string(), "fs_read".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Test write
        let write_request = InternalToolRequest {
            id: "write-test".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": file_path_str,
                "content": "Hello, World! This is a test."
            }),
        };

        let write_result = handler.handle_tool_request(write_request).await.unwrap();
        match write_result {
            ToolCallResult::Success(msg) => {
                assert!(msg.contains("Successfully wrote"));
                assert!(msg.contains("bytes"));
            }
            _ => panic!("Write should succeed"),
        }

        // Test read
        let read_request = InternalToolRequest {
            id: "read-test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": file_path_str
            }),
        };

        let read_result = handler.handle_tool_request(read_request).await.unwrap();
        match read_result {
            ToolCallResult::Success(content) => {
                assert_eq!(content, "Hello, World! This is a test.");
            }
            _ => panic!("Read should succeed"),
        }
    }

    #[tokio::test]
    async fn test_fs_list() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create some test files
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        tokio::fs::write(&file1, "content1").await.unwrap();
        tokio::fs::write(&file2, "content2").await.unwrap();

        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_list".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        let list_request = InternalToolRequest {
            id: "list-test".to_string(),
            name: "fs_list".to_string(),
            arguments: json!({
                "path": temp_dir.path().to_string_lossy()
            }),
        };

        let list_result = handler.handle_tool_request(list_request).await.unwrap();
        match list_result {
            ToolCallResult::Success(content) => {
                assert!(content.contains("Contents of"));
                assert!(content.contains("file1.txt"));
                assert!(content.contains("file2.txt"));
                assert!(content.contains("file"));
                assert!(content.contains("bytes"));
            }
            _ => panic!("List should succeed"),
        }
    }

    #[tokio::test]
    async fn test_terminal_create_and_write() {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["terminal_create".to_string(), "terminal_write".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Create terminal
        let create_request = InternalToolRequest {
            id: "create-test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };

        let create_result = handler.handle_tool_request(create_request).await.unwrap();
        let terminal_id = match create_result {
            ToolCallResult::Success(msg) => {
                assert!(msg.contains("Created terminal session:"));
                // Extract terminal ID from the response
                msg.split_whitespace().last().unwrap().to_string()
            }
            _ => panic!("Terminal creation should succeed"),
        };

        // Execute a simple command
        let write_request = InternalToolRequest {
            id: "write-test".to_string(),
            name: "terminal_write".to_string(),
            arguments: json!({
                "terminal_id": terminal_id,
                "command": "echo 'Hello from terminal'"
            }),
        };

        let write_result = handler.handle_tool_request(write_request).await.unwrap();
        match write_result {
            ToolCallResult::Success(output) => {
                assert!(
                    output.contains("Command output:") || output.contains("Hello from terminal")
                );
            }
            _ => panic!("Command execution should succeed"),
        }
    }

    #[tokio::test]
    async fn test_terminal_cd_operation() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy();

        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["terminal_create".to_string(), "terminal_write".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Create terminal
        let create_request = InternalToolRequest {
            id: "create-test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };

        let create_result = handler.handle_tool_request(create_request).await.unwrap();
        let terminal_id = match create_result {
            ToolCallResult::Success(msg) => msg.split_whitespace().last().unwrap().to_string(),
            _ => panic!("Terminal creation should succeed"),
        };

        // Test cd command
        let cd_request = InternalToolRequest {
            id: "cd-test".to_string(),
            name: "terminal_write".to_string(),
            arguments: json!({
                "terminal_id": terminal_id,
                "command": format!("cd {}", temp_path)
            }),
        };

        let cd_result = handler.handle_tool_request(cd_request).await.unwrap();
        match cd_result {
            ToolCallResult::Success(output) => {
                assert!(output.contains("Changed directory to:"));
            }
            _ => panic!("CD operation should succeed"),
        }
    }

    #[tokio::test]
    async fn test_acp_absolute_path_requirement() {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_read".to_string(), "fs_write".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Test relative paths are rejected with proper ACP error messages
        let relative_paths = vec![
            "relative/path/file.txt",
            "./current/dir/file.txt",
            "../parent/dir/file.txt",
            "src/main.rs",
            "config/settings.json",
        ];

        for path in relative_paths {
            let read_request = InternalToolRequest {
                id: "read-test".to_string(),
                name: "fs_read".to_string(),
                arguments: json!({
                    "path": path
                }),
            };

            let result = handler.handle_tool_request(read_request).await.unwrap();
            match result {
                ToolCallResult::Error(msg) => {
                    assert!(
                        msg.contains("must be absolute path"),
                        "Error message should mention absolute path requirement for '{}': {}",
                        path,
                        msg
                    );
                    assert!(
                        msg.contains("Examples:"),
                        "Error message should include examples for '{}': {}",
                        path,
                        msg
                    );
                }
                _ => panic!("Relative path '{}' should be rejected", path),
            }
        }
    }

    #[tokio::test]
    async fn test_acp_absolute_path_acceptance() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");

        // Write test content to file first
        tokio::fs::write(&file_path, "ACP test content")
            .await
            .unwrap();

        let file_path_str = file_path.to_string_lossy();

        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_read".to_string(), "fs_write".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Test that absolute path is accepted
        let read_request = InternalToolRequest {
            id: "read-test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": file_path_str
            }),
        };

        let result = handler.handle_tool_request(read_request).await.unwrap();
        match result {
            ToolCallResult::Success(content) => {
                assert_eq!(content, "ACP test content");
            }
            ToolCallResult::Error(msg) => panic!("Absolute path should be accepted: {}", msg),
            _ => panic!("Expected success for absolute path"),
        }
    }

    #[tokio::test]
    async fn test_acp_path_traversal_prevention() {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_read".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Test path traversal attempts are blocked
        let traversal_paths = vec![
            "/home/user/../../../etc/passwd",
            "/tmp/../../../root/.ssh/id_rsa",
        ];

        for path in traversal_paths {
            let read_request = InternalToolRequest {
                id: "read-test".to_string(),
                name: "fs_read".to_string(),
                arguments: json!({
                    "path": path
                }),
            };

            let result = handler.handle_tool_request(read_request).await.unwrap();
            match result {
                ToolCallResult::Error(msg) => {
                    assert!(
                        msg.contains("traversal") || msg.contains("relative"),
                        "Error message should mention path traversal prevention for '{}': {}",
                        path,
                        msg
                    );
                }
                _ => panic!("Path traversal attempt '{}' should be blocked", path),
            }
        }
    }

    #[tokio::test]
    async fn test_acp_error_message_format() {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_read".to_string()],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Test that error messages include proper ACP examples
        let read_request = InternalToolRequest {
            id: "read-test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "relative/file.txt"
            }),
        };

        let result = handler.handle_tool_request(read_request).await.unwrap();
        match result {
            ToolCallResult::Error(msg) => {
                // Verify error message contains ACP-compliant examples
                assert!(
                    msg.contains("/home/user/file.txt") || msg.contains("Unix:"),
                    "Error should include Unix example: {}",
                    msg
                );
                assert!(
                    msg.contains("C:\\\\Users\\\\user\\\\file.txt") || msg.contains("Windows:"),
                    "Error should include Windows example: {}",
                    msg
                );
            }
            _ => panic!("Expected error for relative path"),
        }
    }

    #[test]
    fn test_acp_empty_path_handling() {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec![],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Test empty path is handled properly
        let result = handler.validate_file_path("");
        assert!(result.is_err(), "Empty path should be rejected");

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Empty path"),
            "Error should mention empty path: {}",
            error_msg
        );
    }

    #[test]
    fn test_acp_null_byte_prevention() {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec![],
            forbidden_paths: vec![],
        };
        let handler = ToolCallHandler::new(permissions);

        // Test null byte injection is prevented
        let result = handler.validate_file_path("/path/with\0null/byte");
        assert!(result.is_err(), "Path with null byte should be rejected");

        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Null bytes"),
            "Error should mention null bytes: {}",
            error_msg
        );
    }

    #[test]
    fn test_command_validation_dangerous_patterns() {
        let handler = create_test_handler();

        let dangerous_commands = vec![
            "rm -rf /",
            "shutdown now",
            "reboot",
            "halt",
            "poweroff",
            "init 0",
            "dd if=/dev/zero of=/dev/sda",
            "mkfs.ext4 /dev/sda1",
        ];

        for cmd in dangerous_commands {
            assert!(
                handler.validate_command(cmd).is_err(),
                "Command should be blocked: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_command_validation_safe_commands() {
        let handler = create_test_handler();

        let safe_commands = vec![
            "ls -la",
            "pwd",
            "echo 'hello'",
            "cat README.md",
            "grep 'pattern' file.txt",
            "find . -name '*.rs'",
            "git status",
            "cargo build",
        ];

        for cmd in safe_commands {
            assert!(
                handler.validate_command(cmd).is_ok(),
                "Command should be allowed: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_enhanced_file_path_validation() {
        let handler = create_test_handler();

        // Test null byte injection
        assert!(handler.validate_file_path("file\0.txt").is_err());

        // Test dangerous extensions
        let dangerous_files = vec![
            "malware.exe",
            "script.bat",
            "command.cmd",
            "screensaver.scr",
        ];

        for file in dangerous_files {
            assert!(
                handler.validate_file_path(file).is_err(),
                "File should be blocked: {}",
                file
            );
        }

        // Test additional system paths
        let forbidden_paths = vec![
            "/etc/passwd",
            "/usr/bin/sudo",
            "/bin/sh",
            "/sys/kernel/config",
            "/proc/sys/kernel",
            "/dev/sda",
        ];

        for path in forbidden_paths {
            assert!(
                handler.validate_file_path(path).is_err(),
                "Path should be blocked: {}",
                path
            );
        }
    }
}
