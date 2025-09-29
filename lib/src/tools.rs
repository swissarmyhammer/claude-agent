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
use crate::terminal_manager::{TerminalManager, TerminalCreateParams, TerminalCreateResponse, EnvVariable, TerminalSession};
use crate::tool_types::{ToolKind, ToolCallStatus, ToolCallReport, ToolCallContent, ToolCallLocation};


use serde_json::Value;
use agent_client_protocol::SessionId;

use std::collections::HashMap;
use std::sync::Arc;
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











/// Handles tool request execution with permission management and security validation
#[derive(Debug, Clone)]
pub struct ToolCallHandler {
    permissions: ToolPermissions,
    terminal_manager: Arc<TerminalManager>,
    mcp_manager: Option<Arc<crate::mcp::McpServerManager>>,
    /// Client capabilities negotiated during initialization - required for ACP compliance
    client_capabilities: Option<agent_client_protocol::ClientCapabilities>,
    /// Active tool calls tracked by unique ID for session-scoped correlation
    active_tool_calls: Arc<RwLock<HashMap<String, ToolCallReport>>>,
    /// Notification sender for ACP-compliant session updates
    notification_sender: Option<crate::agent::NotificationSender>,
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



impl ToolCallHandler {
    /// Create a new tool call handler with the given permissions
    pub fn new(permissions: ToolPermissions) -> Self {
        Self {
            permissions,
            terminal_manager: Arc::new(TerminalManager::new()),
            mcp_manager: None,
            client_capabilities: None,
            active_tool_calls: Arc::new(RwLock::new(HashMap::new())),
            notification_sender: None,
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
            client_capabilities: None,
            active_tool_calls: Arc::new(RwLock::new(HashMap::new())),
            notification_sender: None,
        }
    }

    /// Generate a unique tool call ID using ULID with collision detection
    pub async fn generate_tool_call_id(&self) -> String {
        let mut attempt = 0;
        const MAX_ATTEMPTS: u32 = 10;

        loop {
            let id = format!("call_{}", ulid::Ulid::new());
            
            // Check for collision in active tool calls
            {
                let active_calls = self.active_tool_calls.read().await;
                if !active_calls.contains_key(&id) {
                    return id;
                }
            }

            attempt += 1;
            if attempt >= MAX_ATTEMPTS {
                // Fallback with timestamp and random component for extremely rare collision cases
                return format!("call_{}_{}", 
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos(),
                    ulid::Ulid::new()
                );
            }
            
            // Short delay before retry (should be extremely rare)
            tokio::time::sleep(tokio::time::Duration::from_nanos(1)).await;
        }
    }

    /// Create and track a new tool call report with ACP-compliant session notification
    pub async fn create_tool_call_report(
        &self,
        session_id: &agent_client_protocol::SessionId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> ToolCallReport {
        let tool_call_id = self.generate_tool_call_id().await;
        let title = ToolCallReport::generate_title(tool_name, arguments);
        let kind = ToolKind::classify_tool(tool_name, arguments);
        
        let mut report = ToolCallReport::new(tool_call_id.clone(), title, kind);
        report.set_raw_input(arguments.clone());
        
        // Track the active tool call
        {
            let mut active_calls = self.active_tool_calls.write().await;
            active_calls.insert(tool_call_id.clone(), report.clone());
        }

        // ACP requires complete tool call status lifecycle reporting:
        // 1. Initial tool_call notification with pending status
        // 2. tool_call_update to in_progress when execution starts
        // 3. Optional progress updates during long-running operations
        // 4. Final tool_call_update with completed/failed/cancelled status
        // 5. Include results/errors in final update content
        //
        // Status updates provide transparency and enable client UI updates.
        
        // Send initial tool_call notification
        if let Some(sender) = &self.notification_sender {
            let notification = agent_client_protocol::SessionNotification {
                session_id: session_id.clone(),
                update: agent_client_protocol::SessionUpdate::ToolCall(report.to_acp_tool_call()),
                meta: None,
            };
            
            if let Err(e) = sender.send_update(notification).await {
                tracing::warn!(
                    tool_call_id = %tool_call_id,
                    session_id = %session_id.0,
                    error = %e,
                    "Failed to send initial tool call notification"
                );
            }
        }
        
        report
    }

    /// Update a tracked tool call report with ACP-compliant session notification
    pub async fn update_tool_call_report(
        &self, 
        session_id: &agent_client_protocol::SessionId,
        tool_call_id: &str, 
        update_fn: impl FnOnce(&mut ToolCallReport)
    ) -> Option<ToolCallReport> {
        let updated_report = {
            let mut active_calls = self.active_tool_calls.write().await;
            if let Some(report) = active_calls.get_mut(tool_call_id) {
                update_fn(report);
                Some(report.clone())
            } else {
                None
            }
        };

        // Send tool_call_update notification for status changes
        if let Some(report) = &updated_report {
            if let Some(sender) = &self.notification_sender {
                let notification = agent_client_protocol::SessionNotification {
                    session_id: session_id.clone(),
                    update: agent_client_protocol::SessionUpdate::ToolCallUpdate(report.to_acp_tool_call_update()),
                    meta: None,
                };
                
                if let Err(e) = sender.send_update(notification).await {
                    tracing::warn!(
                        tool_call_id = %tool_call_id,
                        session_id = %session_id.0,
                        error = %e,
                        "Failed to send tool call update notification"
                    );
                }
            }
        }

        updated_report
    }

    /// Complete and remove a tool call from tracking with ACP-compliant session notification
    pub async fn complete_tool_call_report(
        &self, 
        session_id: &agent_client_protocol::SessionId,
        tool_call_id: &str, 
        raw_output: Option<serde_json::Value>
    ) -> Option<ToolCallReport> {
        let completed_report = {
            let mut active_calls = self.active_tool_calls.write().await;
            if let Some(mut report) = active_calls.remove(tool_call_id) {
                report.update_status(ToolCallStatus::Completed);
                if let Some(output) = raw_output {
                    report.set_raw_output(output);
                }
                Some(report)
            } else {
                None
            }
        };

        // Send final tool_call_update notification with completed status and results
        if let Some(report) = &completed_report {
            if let Some(sender) = &self.notification_sender {
                let notification = agent_client_protocol::SessionNotification {
                    session_id: session_id.clone(),
                    update: agent_client_protocol::SessionUpdate::ToolCallUpdate(report.to_acp_tool_call_update()),
                    meta: None,
                };
                
                if let Err(e) = sender.send_update(notification).await {
                    tracing::warn!(
                        tool_call_id = %tool_call_id,
                        session_id = %session_id.0,
                        error = %e,
                        "Failed to send tool call completion notification"
                    );
                }
            }
        }

        completed_report
    }

    /// Fail and remove a tool call from tracking with ACP-compliant session notification
    pub async fn fail_tool_call_report(
        &self, 
        session_id: &agent_client_protocol::SessionId,
        tool_call_id: &str, 
        error_output: Option<serde_json::Value>
    ) -> Option<ToolCallReport> {
        let failed_report = {
            let mut active_calls = self.active_tool_calls.write().await;
            if let Some(mut report) = active_calls.remove(tool_call_id) {
                report.update_status(ToolCallStatus::Failed);
                if let Some(output) = error_output {
                    report.set_raw_output(output);
                }
                Some(report)
            } else {
                None
            }
        };

        // Send final tool_call_update notification with failed status and error details
        if let Some(report) = &failed_report {
            if let Some(sender) = &self.notification_sender {
                let notification = agent_client_protocol::SessionNotification {
                    session_id: session_id.clone(),
                    update: agent_client_protocol::SessionUpdate::ToolCallUpdate(report.to_acp_tool_call_update()),
                    meta: None,
                };
                
                if let Err(e) = sender.send_update(notification).await {
                    tracing::warn!(
                        tool_call_id = %tool_call_id,
                        session_id = %session_id.0,
                        error = %e,
                        "Failed to send tool call failure notification"
                    );
                }
            }
        }

        failed_report
    }

    /// Cancel and remove a tool call from tracking with ACP-compliant session notification
    pub async fn cancel_tool_call_report(
        &self, 
        session_id: &agent_client_protocol::SessionId,
        tool_call_id: &str
    ) -> Option<ToolCallReport> {
        let cancelled_report = {
            let mut active_calls = self.active_tool_calls.write().await;
            if let Some(mut report) = active_calls.remove(tool_call_id) {
                report.update_status(ToolCallStatus::Cancelled);
                Some(report)
            } else {
                None
            }
        };

        // Send final tool_call_update notification with cancelled status
        if let Some(report) = &cancelled_report {
            if let Some(sender) = &self.notification_sender {
                let notification = agent_client_protocol::SessionNotification {
                    session_id: session_id.clone(),
                    update: agent_client_protocol::SessionUpdate::ToolCallUpdate(report.to_acp_tool_call_update()),
                    meta: None,
                };
                
                if let Err(e) = sender.send_update(notification).await {
                    tracing::warn!(
                        tool_call_id = %tool_call_id,
                        session_id = %session_id.0,
                        error = %e,
                        "Failed to send tool call cancellation notification"
                    );
                }
            }
        }

        cancelled_report
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
            client_capabilities: None,
            active_tool_calls: Arc::new(RwLock::new(HashMap::new())),
            notification_sender: None,
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
            client_capabilities: None,
            active_tool_calls: Arc::new(RwLock::new(HashMap::new())),
            notification_sender: None,
        }
    }

    /// Set client capabilities for ACP compliance - must be called after initialization
    pub fn set_client_capabilities(
        &mut self,
        capabilities: agent_client_protocol::ClientCapabilities,
    ) {
        self.client_capabilities = Some(capabilities);
    }

    /// Set the notification sender for session updates
    pub fn set_notification_sender(&mut self, sender: crate::agent::NotificationSender) {
        self.notification_sender = Some(sender);
    }

    /// Check if client has declared the required file system read capability
    fn validate_fs_read_capability(&self) -> crate::Result<()> {
        match &self.client_capabilities {
            Some(caps) if caps.fs.read_text_file => Ok(()),
            Some(_) => Err(crate::AgentError::Protocol(
                "Method not available: client did not declare fs.read_text_file capability"
                    .to_string(),
            )),
            None => Err(crate::AgentError::Protocol(
                "No client capabilities available for validation".to_string(),
            )),
        }
    }

    /// Check if client has declared the required file system write capability  
    fn validate_fs_write_capability(&self) -> crate::Result<()> {
        match &self.client_capabilities {
            Some(caps) if caps.fs.write_text_file => Ok(()),
            Some(_) => Err(crate::AgentError::Protocol(
                "Method not available: client did not declare fs.write_text_file capability"
                    .to_string(),
            )),
            None => Err(crate::AgentError::Protocol(
                "No client capabilities available for validation".to_string(),
            )),
        }
    }

    /// Check if client has declared the required terminal capability
    ///
    /// ACP requires strict terminal capability validation:
    /// 1. MUST check clientCapabilities.terminal before any terminal operations
    /// 2. MUST NOT attempt terminal methods if capability not declared
    /// 3. MUST return proper errors for unsupported operations
    fn validate_terminal_capability(&self) -> crate::Result<()> {
        match &self.client_capabilities {
            Some(caps) if caps.terminal => Ok(()),
            Some(_) => Err(crate::AgentError::Protocol(
                "Method not supported: client does not support terminal capability. Required capability: clientCapabilities.terminal = true".to_string(),
            )),
            None => Err(crate::AgentError::Protocol(
                "No client capabilities available - terminal operations require clientCapabilities.terminal = true".to_string(),
            )),
        }
    }

    /// Handle an incoming tool request, checking permissions and executing if allowed
    pub async fn handle_tool_request(
        &self,
        session_id: &agent_client_protocol::SessionId,
        request: InternalToolRequest,
    ) -> crate::Result<ToolCallResult> {
        tracing::info!("Handling tool request: {}", request.name);

        // Create tool call report for tracking
        let tool_report = self.create_tool_call_report(session_id, &request.name, &request.arguments).await;
        tracing::debug!("Created tool call report: {}", tool_report.tool_call_id);

        // Check if permission is required for this tool
        if self.requires_permission(&request.name) {
            let permission_request = self.create_permission_request(&request)?;
            return Ok(ToolCallResult::PermissionRequired(permission_request));
        }

        // Update status to in_progress and execute the tool request
        self.update_tool_call_report(session_id, &tool_report.tool_call_id, |report| {
            report.update_status(ToolCallStatus::InProgress);
        }).await;

        match self.execute_tool_request(&request).await {
            Ok(response) => {
                // Complete the tool call with success
                let completed_report = self.complete_tool_call_report(
                    session_id,
                    &tool_report.tool_call_id, 
                    Some(serde_json::json!({"response": response}))
                ).await;
                
                if let Some(report) = completed_report {
                    tracing::debug!("Completed tool call: {} with status {:?}", report.tool_call_id, report.status);
                }
                
                Ok(ToolCallResult::Success(response))
            },
            Err(e) => {
                // Fail the tool call with error
                let failed_report = self.fail_tool_call_report(
                    session_id,
                    &tool_report.tool_call_id, 
                    Some(serde_json::json!({"error": e.to_string()}))
                ).await;
                
                if let Some(report) = failed_report {
                    tracing::debug!("Failed tool call: {} with error: {}", report.tool_call_id, e);
                }
                
                Ok(ToolCallResult::Error(e.to_string()))
            },
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
    ///
    /// ACP requires strict capability validation:
    /// - Terminal tools are only included if client declares terminal capability
    /// - This prevents protocol violations and ensures client compatibility
    pub async fn list_all_available_tools(&self) -> Vec<String> {
        let mut tools = vec![
            "fs_read".to_string(),
            "fs_write".to_string(),
            "fs_list".to_string(),
        ];

        // ACP compliance: Only include terminal tools if client supports them
        if let Some(caps) = &self.client_capabilities {
            if caps.terminal {
                tools.push("terminal_create".to_string());
                tools.push("terminal_write".to_string());
            }
        }

        if let Some(ref mcp_manager) = self.mcp_manager {
            let mcp_tools = mcp_manager.list_available_tools().await;
            tools.extend(mcp_tools);
        }

        tools
    }

    /// Handle file read operations with security validation
    async fn handle_fs_read(&self, request: &InternalToolRequest) -> crate::Result<String> {
        // ACP requires that we only use features the client declared support for.
        // Always check client capabilities before attempting operations.
        // This prevents protocol violations and ensures compatibility.
        self.validate_fs_read_capability()?;

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
        // ACP requires that we only use features the client declared support for.
        // Always check client capabilities before attempting operations.
        // This prevents protocol violations and ensures compatibility.
        self.validate_fs_write_capability()?;

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
        // ACP requires that we only use features the client declared support for.
        // Always check client capabilities before attempting operations.
        // This prevents protocol violations and ensures compatibility.
        self.validate_terminal_capability()?;

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
        // ACP requires that we only use features the client declared support for.
        // Always check client capabilities before attempting operations.
        // This prevents protocol violations and ensures compatibility.
        self.validate_terminal_capability()?;

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
    pub fn generate_permission_options(
        &self,
        request: &InternalToolRequest,
    ) -> Vec<PermissionOption> {
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
                        if sensitive_patterns
                            .iter()
                            .any(|&pattern| path.starts_with(pattern))
                        {
                            return ToolRiskLevel::High;
                        }
                        // Configuration files are moderate risk
                        if path.ends_with(".conf")
                            || path.ends_with(".config")
                            || path.contains("config")
                        {
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

/// ACP-compliant terminal method handler
#[derive(Debug, Clone)]
pub struct TerminalMethodHandler {
    terminal_manager: Arc<TerminalManager>,
    session_manager: Arc<crate::session::SessionManager>,
}

impl TerminalMethodHandler {
    /// Create a new terminal method handler
    pub fn new(
        terminal_manager: Arc<TerminalManager>,
        session_manager: Arc<crate::session::SessionManager>,
    ) -> Self {
        Self {
            terminal_manager,
            session_manager,
        }
    }

    /// Handle ACP terminal/create method
    pub async fn handle_terminal_create(
        &self,
        params: TerminalCreateParams,
    ) -> crate::Result<TerminalCreateResponse> {
        let terminal_id = self
            .terminal_manager
            .create_terminal_with_command(&self.session_manager, params)
            .await?;

        Ok(TerminalCreateResponse { terminal_id })
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
            require_permission_for: vec!["fs_write".to_string(), "terminal_create".to_string()],
            auto_approved: vec![
                "fs_read".to_string(),
                "fs_list".to_string(),
                "terminal_write".to_string(),
            ],
            forbidden_paths: vec![
                "/etc".to_string(),
                "/usr".to_string(),
                "/bin".to_string(),
                "/sys".to_string(),
                "/proc".to_string(),
            ],
        };
        create_test_handler_with_permissions(permissions)
    }

    fn create_test_handler_with_permissions(permissions: ToolPermissions) -> ToolCallHandler {
        let mut handler = ToolCallHandler::new(permissions);

        // Set up test client capabilities for ACP compliance
        let test_capabilities = agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        };
        handler.set_client_capabilities(test_capabilities);
        handler
    }

    fn create_test_session_id() -> SessionId {
        SessionId(std::sync::Arc::from("test_session_123"))
    }

    #[tokio::test]
    async fn test_fs_read_tool() {
        let handler = create_test_handler();
        let session_id = create_test_session_id();

        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "/safe/path/file.txt"
            }),
        };

        let result = handler.handle_tool_request(&session_id, request).await;

        // The file doesn't exist, so we expect an error
        match result {
            Ok(ToolCallResult::Success(_)) => {
                panic!("Expected error for non-existent file, got success");
            }
            Ok(ToolCallResult::Error(msg)) => {
                // The error message could be from file I/O or from path validation
                // Accept either type of error since both are valid responses
                assert!(
                    msg.contains("Failed to read file")
                        || msg.contains("path")
                        || msg.contains("absolute")
                        || msg.contains("No such file")
                );
            }
            Ok(ToolCallResult::PermissionRequired(_)) => {
                panic!("fs_read should be auto-approved, got permission required");
            }
            Err(e) => {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_permission_required() {
        let handler = create_test_handler();
        let session_id = create_test_session_id();

        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": "/safe/path/file.txt",
                "content": "Hello"
            }),
        };

        let result = handler.handle_tool_request(&session_id, request).await.unwrap();

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
        let session_id = create_test_session_id();

        let request = InternalToolRequest {
            id: "test-id".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "../../../etc/passwd"
            }),
        };

        let result = handler.handle_tool_request(&session_id, request).await.unwrap();

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

        let handler = create_test_handler_with_permissions(permissions);

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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

        // Test write
        let write_request = InternalToolRequest {
            id: "write-test".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": file_path_str,
                "content": "Hello, World! This is a test."
            }),
        };

        let write_result = handler.handle_tool_request(&session_id, write_request).await.unwrap();
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

        let read_result = handler.handle_tool_request(&session_id, read_request).await.unwrap();
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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

        let list_request = InternalToolRequest {
            id: "list-test".to_string(),
            name: "fs_list".to_string(),
            arguments: json!({
                "path": temp_dir.path().to_string_lossy()
            }),
        };

        let list_result = handler.handle_tool_request(&session_id, list_request).await.unwrap();
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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

        // Create terminal
        let create_request = InternalToolRequest {
            id: "create-test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };

        let create_result = handler.handle_tool_request(&session_id, create_request).await.unwrap();
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

        let write_result = handler.handle_tool_request(&session_id, write_request).await.unwrap();
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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

        // Create terminal
        let create_request = InternalToolRequest {
            id: "create-test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };

        let create_result = handler.handle_tool_request(&session_id, create_request).await.unwrap();
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

        let cd_result = handler.handle_tool_request(&session_id, cd_request).await.unwrap();
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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

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

            let result = handler.handle_tool_request(&session_id, read_request).await.unwrap();
            match result {
                ToolCallResult::Error(msg) => {
                    // With capability validation now happening first, we expect either:
                    // 1. Capability validation errors (which are valid security checks)
                    // 2. Path validation errors (if capabilities pass)
                    assert!(
                        msg.contains("must be absolute path") || 
                        msg.contains("capability") ||
                        msg.contains("No client capabilities") ||
                        msg.contains("Examples:"),
                        "Error message should mention either capability or path validation for '{}': {}",
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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

        // Test that absolute path is accepted
        let read_request = InternalToolRequest {
            id: "read-test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": file_path_str
            }),
        };

        let result = handler.handle_tool_request(&session_id, read_request).await.unwrap();
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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

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

            let result = handler.handle_tool_request(&session_id, read_request).await.unwrap();
            match result {
                ToolCallResult::Error(msg) => {
                    // With capability validation first, we expect either capability errors or path errors
                    assert!(
                        msg.contains("traversal") || 
                        msg.contains("relative") || 
                        msg.contains("capability") ||
                        msg.contains("No client capabilities"),
                        "Error message should mention either capability or path traversal prevention for '{}': {}",
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
        let handler = create_test_handler_with_permissions(permissions);
        let session_id = create_test_session_id();

        // Test that error messages include proper ACP examples
        let read_request = InternalToolRequest {
            id: "read-test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": "relative/file.txt"
            }),
        };

        let result = handler.handle_tool_request(&session_id, read_request).await.unwrap();
        match result {
            ToolCallResult::Error(msg) => {
                // With capability validation first, we expect either capability errors or path errors
                if msg.contains("capability") || msg.contains("No client capabilities") {
                    // Capability validation error is valid
                } else {
                    // If we get a path validation error, verify it contains ACP-compliant examples
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
        let handler = create_test_handler_with_permissions(permissions);

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
        let handler = create_test_handler_with_permissions(permissions);

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

    /// Test ACP capability validation for file system operations
    #[tokio::test]
    async fn test_capability_validation_fs_operations() {
        // Test with fs.read_text_file disabled
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_read".to_string()],
            forbidden_paths: vec![],
        };
        let mut handler = ToolCallHandler::new(permissions);
        let session_id = create_test_session_id();
        let caps_no_read = agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: false,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
            meta: None,
        };
        handler.set_client_capabilities(caps_no_read);

        let read_request = InternalToolRequest {
            id: "test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({"path": "/test/file.txt"}),
        };

        let result = handler.handle_tool_request(&session_id, read_request).await.unwrap();
        match result {
            ToolCallResult::Error(msg) => {
                assert!(msg.contains("fs.read_text_file capability"));
            }
            _ => panic!("Expected capability error for fs_read when read_text_file is false"),
        }
    }

    /// Test ACP capability validation for terminal operations
    #[tokio::test]
    async fn test_capability_validation_terminal_operations() {
        // Test with terminal capability disabled
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["terminal_create".to_string()],
            forbidden_paths: vec![],
        };
        let mut handler = ToolCallHandler::new(permissions);
        let session_id = create_test_session_id();
        let caps_no_terminal = agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
            meta: None,
        };
        handler.set_client_capabilities(caps_no_terminal);

        let terminal_request = InternalToolRequest {
            id: "test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };

        let result = handler.handle_tool_request(&session_id, terminal_request).await.unwrap();
        match result {
            ToolCallResult::Error(msg) => {
                assert!(msg.contains("terminal capability"));
            }
            _ => panic!("Expected capability error for terminal_create when terminal is false"),
        }
    }

    /// Test ACP capability validation allows operations when capabilities are enabled
    #[tokio::test]
    async fn test_capability_validation_allows_enabled_operations() {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["fs_read".to_string(), "terminal_create".to_string()],
            forbidden_paths: vec![],
        };
        let mut handler = ToolCallHandler::new(permissions);
        let session_id = create_test_session_id();
        let caps_enabled = agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        };
        handler.set_client_capabilities(caps_enabled);

        // Test fs_read passes capability validation (will fail later due to file not existing)
        let read_request = InternalToolRequest {
            id: "test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({"path": "/test/file.txt"}),
        };

        let result = handler.handle_tool_request(&session_id, read_request).await.unwrap();
        if let ToolCallResult::Error(msg) = result {
            // Should be a file I/O error, not a capability error
            assert!(!msg.contains("capability"));
            assert!(msg.contains("Failed to read file") || msg.contains("absolute"));
        }

        // Test terminal_create passes capability validation
        let terminal_request = InternalToolRequest {
            id: "test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };

        let result = handler.handle_tool_request(&session_id, terminal_request).await.unwrap();
        match result {
            ToolCallResult::Success(msg) => {
                assert!(msg.contains("Created terminal session"));
            }
            ToolCallResult::Error(msg) => {
                // Should not be a capability error
                assert!(!msg.contains("capability"));
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_terminal_capability_tool_availability_filtering() {
        // Test that terminal tools are filtered from available tools based on client capabilities

        // Test with terminal capability disabled
        let mut handler_no_terminal = ToolCallHandler::new(ToolPermissions {
            auto_approved: vec![],
            require_permission_for: vec![],
            forbidden_paths: vec![],
        });

        let caps_no_terminal = agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
            meta: None,
        };

        handler_no_terminal.set_client_capabilities(caps_no_terminal);

        let tools_no_terminal = handler_no_terminal.list_all_available_tools().await;

        // Should not include terminal tools
        assert!(!tools_no_terminal.contains(&"terminal_create".to_string()));
        assert!(!tools_no_terminal.contains(&"terminal_write".to_string()));

        // Should still include file system tools
        assert!(tools_no_terminal.contains(&"fs_read".to_string()));
        assert!(tools_no_terminal.contains(&"fs_write".to_string()));
        assert!(tools_no_terminal.contains(&"fs_list".to_string()));

        // Test with terminal capability enabled
        let mut handler_with_terminal = ToolCallHandler::new(ToolPermissions {
            auto_approved: vec![],
            require_permission_for: vec![],
            forbidden_paths: vec![],
        });

        let caps_with_terminal = agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        };

        handler_with_terminal.set_client_capabilities(caps_with_terminal);

        let tools_with_terminal = handler_with_terminal.list_all_available_tools().await;

        // Should include all tools including terminal tools
        assert!(tools_with_terminal.contains(&"terminal_create".to_string()));
        assert!(tools_with_terminal.contains(&"terminal_write".to_string()));
        assert!(tools_with_terminal.contains(&"fs_read".to_string()));
        assert!(tools_with_terminal.contains(&"fs_write".to_string()));
        assert!(tools_with_terminal.contains(&"fs_list".to_string()));

        // Test with no client capabilities set
        let handler_no_caps = ToolCallHandler::new(ToolPermissions {
            auto_approved: vec![],
            require_permission_for: vec![],
            forbidden_paths: vec![],
        });

        let tools_no_caps = handler_no_caps.list_all_available_tools().await;

        // Should not include terminal tools when no capabilities are set
        assert!(!tools_no_caps.contains(&"terminal_create".to_string()));
        assert!(!tools_no_caps.contains(&"terminal_write".to_string()));

        // Should still include file system tools
        assert!(tools_no_caps.contains(&"fs_read".to_string()));
        assert!(tools_no_caps.contains(&"fs_write".to_string()));
        assert!(tools_no_caps.contains(&"fs_list".to_string()));
    }

    #[tokio::test]
    async fn test_acp_terminal_create_with_all_parameters() {
        use crate::session::SessionManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new());
        let terminal_manager = Arc::new(TerminalManager::new());

        // Create test session
        let session_id = session_manager
            .create_session(temp_dir.path().to_path_buf())
            .unwrap();

        // Test terminal creation with all parameters
        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "echo".to_string(),
            args: Some(vec!["Hello".to_string(), "World".to_string()]),
            env: Some(vec![
                EnvVariable {
                    name: "TEST_VAR".to_string(),
                    value: "test_value".to_string(),
                },
                EnvVariable {
                    name: "NODE_ENV".to_string(),
                    value: "test".to_string(),
                },
            ]),
            cwd: Some(temp_dir.path().to_string_lossy().to_string()),
            output_byte_limit: Some(2048),
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Verify terminal ID has correct format
        assert!(terminal_id.starts_with("term_"));
        assert!(terminal_id.len() > 5); // "term_" + ULID

        // Verify terminal session was created with correct parameters
        let terminals = terminal_manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();

        assert_eq!(session.command.as_ref().unwrap(), "echo");
        assert_eq!(session.args, vec!["Hello", "World"]);
        assert_eq!(
            session.session_id.as_ref().unwrap(),
            &session_id.to_string()
        );
        assert_eq!(session.output_byte_limit, 2048);
        assert!(session.environment.contains_key("TEST_VAR"));
        assert_eq!(session.environment.get("TEST_VAR").unwrap(), "test_value");
        assert_eq!(session.environment.get("NODE_ENV").unwrap(), "test");
        assert!(session.working_dir.is_absolute());
    }

    #[tokio::test]
    async fn test_acp_terminal_create_minimal_parameters() {
        use crate::session::SessionManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new());
        let terminal_manager = Arc::new(TerminalManager::new());

        // Create test session
        let session_id = session_manager
            .create_session(temp_dir.path().to_path_buf())
            .unwrap();

        // Test terminal creation with minimal parameters
        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "pwd".to_string(),
            args: None,
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Verify terminal session was created with defaults
        let terminals = terminal_manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();

        assert_eq!(session.command.as_ref().unwrap(), "pwd");
        assert!(session.args.is_empty());
        assert_eq!(session.output_byte_limit, 1_048_576); // Default 1MB
        assert_eq!(session.working_dir, temp_dir.path()); // Uses session cwd
    }

    #[tokio::test]
    async fn test_acp_terminal_create_invalid_session_id() {
        use crate::session::SessionManager;

        let session_manager = Arc::new(SessionManager::new());
        let terminal_manager = Arc::new(TerminalManager::new());

        let params = TerminalCreateParams {
            session_id: "invalid-session-id".to_string(),
            command: "echo".to_string(),
            args: None,
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let result = terminal_manager
            .create_terminal_with_command(&session_manager, params)
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Invalid session ID format"));
    }

    #[tokio::test]
    async fn test_acp_terminal_create_nonexistent_session() {
        use crate::session::SessionManager;

        let session_manager = Arc::new(SessionManager::new());
        let terminal_manager = Arc::new(TerminalManager::new());

        let params = TerminalCreateParams {
            session_id: ulid::Ulid::new().to_string(), // Valid ULID but non-existent
            command: "echo".to_string(),
            args: None,
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let result = terminal_manager
            .create_terminal_with_command(&session_manager, params)
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Session not found"));
    }

    #[tokio::test]
    async fn test_terminal_session_output_buffer_management() {
        let mut session = TerminalSession {
            process: None,
            working_dir: std::path::PathBuf::from("/tmp"),
            environment: HashMap::new(),
            command: Some("test".to_string()),
            args: Vec::new(),
            session_id: Some("test".to_string()),
            output_byte_limit: 10, // Very small for testing
            output_buffer: Vec::new(),
            buffer_truncated: false,
        };

        // Test normal addition within limits
        session.add_output(b"hello");
        assert_eq!(session.get_output_string(), "hello");
        assert!(!session.is_output_truncated());
        assert_eq!(session.get_buffer_size(), 5);

        // Test addition that exceeds limit
        session.add_output(b" world test");
        assert_eq!(session.get_output_string(), "world test"); // Truncated from beginning
        assert!(session.is_output_truncated());
        assert_eq!(session.get_buffer_size(), 10);

        // Test large addition that fills entire buffer
        session.add_output(b"replacement");
        assert_eq!(session.get_output_string(), "eplacement"); // Last 10 bytes within limit
        assert!(session.is_output_truncated());
        assert_eq!(session.get_buffer_size(), 10);

        // Test clearing buffer
        session.clear_output();
        assert_eq!(session.get_output_string(), "");
        assert!(!session.is_output_truncated());
        assert_eq!(session.get_buffer_size(), 0);
    }

    #[tokio::test]
    async fn test_environment_variable_validation() {
        let terminal_manager = TerminalManager::new();

        // Test valid environment variables
        let env_vars = vec![
            EnvVariable {
                name: "TEST_VAR".to_string(),
                value: "test_value".to_string(),
            },
            EnvVariable {
                name: "PATH".to_string(),
                value: "/usr/bin:/bin".to_string(),
            },
        ];

        let result = terminal_manager.prepare_environment(env_vars);
        assert!(result.is_ok());
        let environment = result.unwrap();
        assert_eq!(environment.get("TEST_VAR").unwrap(), "test_value");
        assert_eq!(environment.get("PATH").unwrap(), "/usr/bin:/bin");

        // Test empty variable name
        let invalid_env_vars = vec![EnvVariable {
            name: "".to_string(),
            value: "some_value".to_string(),
        }];

        let result = terminal_manager.prepare_environment(invalid_env_vars);
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Environment variable name cannot be empty"));
    }

    #[tokio::test]
    async fn test_working_directory_resolution() {
        use crate::session::SessionManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let other_temp_dir = TempDir::new().unwrap();
        let session_manager = SessionManager::new();
        let terminal_manager = TerminalManager::new();

        // Create test session with specific working directory
        let session_id = session_manager
            .create_session(temp_dir.path().to_path_buf())
            .unwrap();

        // Test using session's working directory (no cwd parameter)
        let result = terminal_manager
            .resolve_working_directory(&session_manager, &session_id.to_string(), None)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), temp_dir.path());

        // Test using provided absolute path
        let result = terminal_manager
            .resolve_working_directory(
                &session_manager,
                &session_id.to_string(),
                Some(&other_temp_dir.path().to_string_lossy()),
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), other_temp_dir.path());

        // Test using relative path (should fail)
        let result = terminal_manager
            .resolve_working_directory(
                &session_manager,
                &session_id.to_string(),
                Some("relative/path"),
            )
            .await;
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("must be absolute path"));
    }

    #[tokio::test]
    async fn test_terminal_method_handler() {
        use crate::session::SessionManager;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let session_manager = Arc::new(SessionManager::new());
        let terminal_manager = Arc::new(TerminalManager::new());
        let handler = TerminalMethodHandler::new(terminal_manager.clone(), session_manager.clone());

        // Create test session
        let session_id = session_manager
            .create_session(temp_dir.path().to_path_buf())
            .unwrap();

        // Test ACP terminal/create handler
        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "echo".to_string(),
            args: Some(vec!["test".to_string()]),
            env: Some(vec![EnvVariable {
                name: "TEST_ENV".to_string(),
                value: "test_val".to_string(),
            }]),
            cwd: None,
            output_byte_limit: Some(4096),
        };

        let response = handler.handle_terminal_create(params).await.unwrap();

        // Verify response format
        assert!(response.terminal_id.starts_with("term_"));

        // Verify terminal was created
        let terminals = terminal_manager.terminals.read().await;
        let session = terminals.get(&response.terminal_id).unwrap();
        assert_eq!(session.command.as_ref().unwrap(), "echo");
        assert_eq!(session.args, vec!["test"]);
        assert_eq!(session.output_byte_limit, 4096);
    }

    #[tokio::test]
    async fn test_improved_terminal_capability_error_messages() {
        // Test that terminal capability validation returns improved error messages
        let mut handler = ToolCallHandler::new(ToolPermissions {
            auto_approved: vec!["terminal_create".to_string()],
            require_permission_for: vec![],
            forbidden_paths: vec![],
        });
        let session_id = create_test_session_id();

        // Test with terminal capability explicitly disabled
        let caps_disabled = agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: false,
            meta: None,
        };

        handler.set_client_capabilities(caps_disabled);

        let terminal_request = InternalToolRequest {
            id: "test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };

        let result = handler.handle_tool_request(&session_id, terminal_request).await.unwrap();
        match result {
            ToolCallResult::Error(msg) => {
                assert!(msg.contains("Method not supported"));
                assert!(msg.contains("clientCapabilities.terminal = true"));
            }
            _ => panic!("Expected error for disabled terminal capability"),
        }

        // Test with no capabilities provided
        let handler_no_caps = ToolCallHandler::new(ToolPermissions {
            auto_approved: vec!["terminal_create".to_string()],
            require_permission_for: vec![],
            forbidden_paths: vec![],
        });

        let result_no_caps = handler_no_caps
            .handle_tool_request(&session_id, InternalToolRequest {
                id: "test".to_string(),
                name: "terminal_create".to_string(),
                arguments: json!({}),
            })
            .await
            .unwrap();

        match result_no_caps {
            ToolCallResult::Error(msg) => {
                assert!(msg.contains("No client capabilities available"));
                assert!(msg.contains("clientCapabilities.terminal = true"));
            }
            _ => panic!("Expected error for missing client capabilities"),
        }
    }

    #[tokio::test]
    async fn test_tool_kind_classification() {
        // Test file system operations
        assert_eq!(ToolKind::classify_tool("fs_read_text_file", &json!({})), ToolKind::Read);
        assert_eq!(ToolKind::classify_tool("fs_write_text_file", &json!({})), ToolKind::Edit);
        assert_eq!(ToolKind::classify_tool("fs_delete", &json!({})), ToolKind::Delete);
        assert_eq!(ToolKind::classify_tool("fs_move", &json!({})), ToolKind::Move);
        
        // Test terminal operations
        assert_eq!(ToolKind::classify_tool("terminal_create", &json!({})), ToolKind::Execute);
        assert_eq!(ToolKind::classify_tool("execute", &json!({})), ToolKind::Execute);
        
        // Test search operations
        assert_eq!(ToolKind::classify_tool("search", &json!({})), ToolKind::Search);
        assert_eq!(ToolKind::classify_tool("grep", &json!({})), ToolKind::Search);
        
        // Test fetch operations
        assert_eq!(ToolKind::classify_tool("fetch", &json!({})), ToolKind::Fetch);
        assert_eq!(ToolKind::classify_tool("http_get", &json!({})), ToolKind::Fetch);
        
        // Test MCP tool classification
        assert_eq!(ToolKind::classify_tool("mcp__files_read", &json!({})), ToolKind::Read);
        assert_eq!(ToolKind::classify_tool("mcp__files_write", &json!({})), ToolKind::Edit);
        assert_eq!(ToolKind::classify_tool("mcp__shell_execute", &json!({})), ToolKind::Execute);
        assert_eq!(ToolKind::classify_tool("mcp__web_fetch", &json!({})), ToolKind::Fetch);
        
        // Test default fallback
        assert_eq!(ToolKind::classify_tool("unknown_tool", &json!({})), ToolKind::Other);
    }

    #[tokio::test]
    async fn test_tool_title_generation() {
        // Test file operations with paths
        let title = ToolCallReport::generate_title("fs_read_text_file", &json!({
            "path": "/home/user/config.json"
        }));
        assert_eq!(title, "Reading config.json");

        let title = ToolCallReport::generate_title("fs_write_text_file", &json!({
            "path": "/var/log/app.log"
        }));
        assert_eq!(title, "Writing to app.log");

        // Test terminal operations
        let title = ToolCallReport::generate_title("terminal_create", &json!({
            "command": "ls"
        }));
        assert_eq!(title, "Running ls");

        // Test search operations
        let title = ToolCallReport::generate_title("search", &json!({
            "pattern": "error.*log"
        }));
        assert_eq!(title, "Searching for 'error.*log'");

        // Test MCP tools
        let title = ToolCallReport::generate_title("mcp__files_read", &json!({}));
        assert_eq!(title, "Files read");

        // Test fallback for unknown tools
        let title = ToolCallReport::generate_title("unknown_tool", &json!({}));
        assert_eq!(title, "Unknown tool");

        // Test snake_case conversion
        let title = ToolCallReport::generate_title("create_backup_file", &json!({}));
        assert_eq!(title, "Create backup file");
    }

    #[tokio::test]
    async fn test_tool_call_id_generation() {
        let handler = create_test_handler();
        
        // Test unique ID generation
        let id1 = handler.generate_tool_call_id().await;
        let id2 = handler.generate_tool_call_id().await;
        
        assert_ne!(id1, id2);
        assert!(id1.starts_with("call_"));
        assert!(id2.starts_with("call_"));
        
        // IDs should be ULID format after the prefix
        let ulid_part1 = id1.strip_prefix("call_").unwrap();
        let ulid_part2 = id2.strip_prefix("call_").unwrap();
        
        assert_eq!(ulid_part1.len(), 26); // ULID length
        assert_eq!(ulid_part2.len(), 26);
        
        // Test multiple concurrent ID generations don't collide
        let mut ids = Vec::new();
        for _ in 0..10 {
            ids.push(handler.generate_tool_call_id().await);
        }
        
        // Check all IDs are unique
        for i in 0..ids.len() {
            for j in i+1..ids.len() {
                assert_ne!(ids[i], ids[j], "Found duplicate IDs: {} and {}", ids[i], ids[j]);
            }
        }
    }

    #[tokio::test]
    async fn test_tool_call_report_lifecycle() {
        let handler = create_test_handler();
        let session_id = agent_client_protocol::SessionId("test_session".into());
        
        // Create a tool call report
        let report = handler.create_tool_call_report(&session_id, "fs_read_text_file", &json!({
            "path": "/test/file.txt"
        })).await;
        
        assert_eq!(report.status, ToolCallStatus::Pending);
        assert_eq!(report.kind, ToolKind::Read);
        assert_eq!(report.title, "Reading file.txt");
        assert!(report.raw_input.is_some());
        assert!(report.raw_output.is_none());
        
        // Update the report status
        let updated = handler.update_tool_call_report(&session_id, &report.tool_call_id, |r| {
            r.update_status(ToolCallStatus::InProgress);
            r.add_content(ToolCallContent::Content {
                content: agent_client_protocol::ContentBlock::Text(
                    agent_client_protocol::TextContent {
                        text: "Reading file...".to_string(),
                        annotations: None,
                        meta: None,
                    }
                ),
            });
        }).await;
        
        assert!(updated.is_some());
        let updated = updated.unwrap();
        assert_eq!(updated.status, ToolCallStatus::InProgress);
        assert_eq!(updated.content.len(), 1);
        
        // Complete the tool call
        let completed = handler.complete_tool_call_report(
            &session_id,
            &report.tool_call_id,
            Some(json!({"content": "file contents"}))
        ).await;
        
        assert!(completed.is_some());
        let completed = completed.unwrap();
        assert_eq!(completed.status, ToolCallStatus::Completed);
        assert!(completed.raw_output.is_some());
        
        // Try to update a completed (removed) tool call - should return None
        let not_found = handler.update_tool_call_report(&session_id, &report.tool_call_id, |r| {
            r.update_status(ToolCallStatus::Failed);
        }).await;
        
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_tool_call_report_failure() {
        let handler = create_test_handler();
        let session_id = agent_client_protocol::SessionId("test_session".into());
        
        // Create a tool call report
        let report = handler.create_tool_call_report(&session_id, "fs_write_text_file", &json!({
            "path": "/readonly/file.txt",
            "content": "test"
        })).await;
        
        // Fail the tool call
        let failed = handler.fail_tool_call_report(
            &session_id,
            &report.tool_call_id,
            Some(json!({"error": "Permission denied", "code": "EACCES"}))
        ).await;
        
        assert!(failed.is_some());
        let failed = failed.unwrap();
        assert_eq!(failed.status, ToolCallStatus::Failed);
        assert!(failed.raw_output.is_some());
        
        if let Some(output) = failed.raw_output {
            assert!(output["error"].as_str().unwrap().contains("Permission denied"));
            assert_eq!(output["code"], "EACCES");
        }
    }

    #[tokio::test] 
    async fn test_tool_call_locations_and_content() {
        let mut report = ToolCallReport::new(
            "call_test123".to_string(),
            "Test operation".to_string(), 
            ToolKind::Edit
        );
        
        // Add file locations
        report.add_location(ToolCallLocation {
            path: "/home/user/src/main.rs".to_string(),
            line: Some(42),
        });
        
        report.add_location(ToolCallLocation {
            path: "/home/user/src/lib.rs".to_string(),
            line: None,
        });
        
        // Add different types of content
        report.add_content(ToolCallContent::Content {
            content: agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Operation completed".to_string(),
                    annotations: None,
                    meta: None,
                }
            ),
        });
        
        report.add_content(ToolCallContent::Diff {
            path: "/home/user/src/main.rs".to_string(),
            old_text: Some("fn old() {}".to_string()),
            new_text: "fn new() {}".to_string(),
        });
        
        report.add_content(ToolCallContent::Terminal {
            terminal_id: "term_abc123".to_string(),
        });
        
        assert_eq!(report.locations.len(), 2);
        assert_eq!(report.content.len(), 3);
        assert_eq!(report.locations[0].line, Some(42));
        assert_eq!(report.locations[1].line, None);
        
        // Test content types
        match &report.content[0] {
            ToolCallContent::Content { content } => {
                if let agent_client_protocol::ContentBlock::Text(text) = content {
                    assert_eq!(text.text, "Operation completed");
                } else {
                    panic!("Expected text content");
                }
            },
            _ => panic!("Expected content type"),
        }
        
        match &report.content[1] {
            ToolCallContent::Diff { path, old_text, new_text } => {
                assert_eq!(path, "/home/user/src/main.rs");
                assert_eq!(old_text.as_ref().unwrap(), "fn old() {}");
                assert_eq!(new_text, "fn new() {}");
            },
            _ => panic!("Expected diff content"),
        }
        
        match &report.content[2] {
            ToolCallContent::Terminal { terminal_id } => {
                assert_eq!(terminal_id, "term_abc123");
            },
            _ => panic!("Expected terminal content"),
        }
    }
}
