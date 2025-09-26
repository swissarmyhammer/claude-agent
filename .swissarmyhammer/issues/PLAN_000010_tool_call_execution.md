# Tool Call Execution Implementation

Refer to plan.md

## Goal
Implement actual tool call execution for file system operations and terminal commands with security validation.

## Tasks

### 1. File System Operations (`lib/src/tools.rs`)

```rust
use tokio::fs;
use std::path::Path;

impl ToolCallHandler {
    async fn handle_fs_read(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        let args = self.parse_tool_args(&tool_call.arguments)?;
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'path' argument".to_string()))?;
        
        tracing::debug!("Reading file: {}", path);
        
        // Validate path security
        self.validate_file_path(path)?;
        
        // Read file
        match fs::read_to_string(path).await {
            Ok(content) => {
                tracing::info!("Successfully read {} bytes from {}", content.len(), path);
                
                Ok(ToolCallContent {
                    tool_call_id: tool_call.id.clone(),
                    content: vec![agent_client_protocol::ContentBlock::Text {
                        text: content,
                    }],
                })
            }
            Err(e) => {
                let error_msg = format!("Failed to read file {}: {}", path, e);
                tracing::error!("{}", error_msg);
                
                Ok(ToolCallContent {
                    tool_call_id: tool_call.id.clone(),
                    content: vec![agent_client_protocol::ContentBlock::Text {
                        text: format!("Error: {}", error_msg),
                    }],
                })
            }
        }
    }
    
    async fn handle_fs_write(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        let args = self.parse_tool_args(&tool_call.arguments)?;
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'path' argument".to_string()))?;
        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'content' argument".to_string()))?;
        
        tracing::debug!("Writing to file: {} ({} bytes)", path, content.len());
        
        // Validate path security
        self.validate_file_path(path)?;
        
        // Create parent directories if they don't exist
        if let Some(parent) = Path::new(path).parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    let error_msg = format!("Failed to create parent directories for {}: {}", path, e);
                    tracing::error!("{}", error_msg);
                    return Ok(ToolCallContent {
                        tool_call_id: tool_call.id.clone(),
                        content: vec![agent_client_protocol::ContentBlock::Text {
                            text: format!("Error: {}", error_msg),
                        }],
                    });
                }
            }
        }
        
        // Write file
        match fs::write(path, content).await {
            Ok(()) => {
                tracing::info!("Successfully wrote {} bytes to {}", content.len(), path);
                
                Ok(ToolCallContent {
                    tool_call_id: tool_call.id.clone(),
                    content: vec![agent_client_protocol::ContentBlock::Text {
                        text: format!("Successfully wrote {} bytes to {}", content.len(), path),
                    }],
                })
            }
            Err(e) => {
                let error_msg = format!("Failed to write file {}: {}", path, e);
                tracing::error!("{}", error_msg);
                
                Ok(ToolCallContent {
                    tool_call_id: tool_call.id.clone(),
                    content: vec![agent_client_protocol::ContentBlock::Text {
                        text: format!("Error: {}", error_msg),
                    }],
                })
            }
        }
    }
    
    async fn handle_fs_list(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        let args = self.parse_tool_args(&tool_call.arguments)?;
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        
        tracing::debug!("Listing directory: {}", path);
        
        self.validate_file_path(path)?;
        
        match fs::read_dir(path).await {
            Ok(mut entries) => {
                let mut files = Vec::new();
                
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        let file_type = if metadata.is_dir() { "directory" } else { "file" };
                        let size = if metadata.is_file() { metadata.len() } else { 0 };
                        
                        files.push(format!(
                            "{} ({}, {} bytes)",
                            entry.file_name().to_string_lossy(),
                            file_type,
                            size
                        ));
                    }
                }
                
                let content = if files.is_empty() {
                    format!("Directory {} is empty", path)
                } else {
                    format!("Contents of {}:\n{}", path, files.join("\n"))
                };
                
                Ok(ToolCallContent {
                    tool_call_id: tool_call.id.clone(),
                    content: vec![agent_client_protocol::ContentBlock::Text {
                        text: content,
                    }],
                })
            }
            Err(e) => {
                let error_msg = format!("Failed to list directory {}: {}", path, e);
                tracing::error!("{}", error_msg);
                
                Ok(ToolCallContent {
                    tool_call_id: tool_call.id.clone(),
                    content: vec![agent_client_protocol::ContentBlock::Text {
                        text: format!("Error: {}", error_msg),
                    }],
                })
            }
        }
    }
}
```

### 2. Terminal Operations

```rust
use tokio::process::{Command, Child};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct TerminalManager {
    terminals: Arc<RwLock<HashMap<String, TerminalSession>>>,
}

pub struct TerminalSession {
    id: String,
    process: Option<Child>,
    working_dir: std::path::PathBuf,
    environment: HashMap<String, String>,
    created_at: std::time::SystemTime,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            terminals: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn create_terminal(&self, working_dir: Option<String>) -> crate::Result<String> {
        let terminal_id = uuid::Uuid::new_v4().to_string();
        let working_dir = working_dir
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
        
        let session = TerminalSession {
            id: terminal_id.clone(),
            process: None,
            working_dir,
            environment: std::env::vars().collect(),
            created_at: std::time::SystemTime::now(),
        };
        
        let mut terminals = self.terminals.write().await;
        terminals.insert(terminal_id.clone(), session);
        
        tracing::info!("Created terminal session: {}", terminal_id);
        Ok(terminal_id)
    }
    
    pub async fn execute_command(&self, terminal_id: &str, command: &str) -> crate::Result<String> {
        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id)
            .ok_or_else(|| crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id)))?;
        
        tracing::info!("Executing command in terminal {}: {}", terminal_id, command);
        
        // Parse command and arguments
        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Err(crate::AgentError::ToolExecution("Empty command".to_string()));
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
            .map_err(|e| crate::AgentError::ToolExecution(format!("Failed to execute command: {}", e)))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        let result = if output.status.success() {
            if stdout.is_empty() {
                format!("Command completed successfully (exit code: 0)")
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
        
        tracing::info!("Command completed with exit code: {:?}", output.status.code());
        Ok(result)
    }
    
    pub async fn change_directory(&self, terminal_id: &str, path: &str) -> crate::Result<String> {
        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id)
            .ok_or_else(|| crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id)))?;
        
        let new_path = if Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            session.working_dir.join(path)
        };
        
        if new_path.exists() && new_path.is_dir() {
            session.working_dir = new_path.canonicalize()
                .map_err(|e| crate::AgentError::ToolExecution(format!("Failed to resolve path: {}", e)))?;
            
            tracing::info!("Changed directory to: {}", session.working_dir.display());
            Ok(format!("Changed directory to: {}", session.working_dir.display()))
        } else {
            Err(crate::AgentError::ToolExecution(format!("Directory does not exist: {}", path)))
        }
    }
    
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

impl ToolCallHandler {
    pub fn new_with_terminal_manager(permissions: ToolPermissions) -> Self {
        Self {
            permissions,
            terminal_manager: Arc::new(TerminalManager::new()),
        }
    }
    
    async fn handle_terminal_create(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        let args = self.parse_tool_args(&tool_call.arguments)?;
        let working_dir = args.get("working_dir").and_then(|v| v.as_str());
        
        let terminal_id = self.terminal_manager.create_terminal(working_dir.map(String::from)).await?;
        
        Ok(ToolCallContent {
            tool_call_id: tool_call.id.clone(),
            content: vec![agent_client_protocol::ContentBlock::Text {
                text: format!("Created terminal session: {}", terminal_id),
            }],
        })
    }
    
    async fn handle_terminal_write(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        let args = self.parse_tool_args(&tool_call.arguments)?;
        let terminal_id = args.get("terminal_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'terminal_id' argument".to_string()))?;
        let command = args.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::AgentError::ToolExecution("Missing 'command' argument".to_string()))?;
        
        // Check if this is a directory change command
        if command.trim().starts_with("cd ") {
            let path = command.trim().strip_prefix("cd ").unwrap_or("").trim();
            let result = self.terminal_manager.change_directory(terminal_id, path).await?;
            
            return Ok(ToolCallContent {
                tool_call_id: tool_call.id.clone(),
                content: vec![agent_client_protocol::ContentBlock::Text {
                    text: result,
                }],
            });
        }
        
        let result = self.terminal_manager.execute_command(terminal_id, command).await?;
        
        Ok(ToolCallContent {
            tool_call_id: tool_call.id.clone(),
            content: vec![agent_client_protocol::ContentBlock::Text {
                text: result,
            }],
        })
    }
}
```

### 3. Enhanced Security Validation

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
        
        // Check for null bytes
        if path.to_string_lossy().contains('\0') {
            return Err(crate::AgentError::ToolExecution(
                "Null bytes in path not allowed".to_string()
            ));
        }
        
        // Check for absolute paths outside allowed directories
        if path.is_absolute() {
            let forbidden_prefixes = ["/etc", "/usr", "/bin", "/sys", "/proc", "/dev"];
            let path_str = path.to_string_lossy();
            
            for prefix in &forbidden_prefixes {
                if path_str.starts_with(prefix) {
                    return Err(crate::AgentError::ToolExecution(
                        format!("Access to {} is forbidden", prefix)
                    ));
                }
            }
        }
        
        // Check file extension restrictions for write operations
        if let Some(ext) = path.extension() {
            let dangerous_extensions = ["exe", "bat", "cmd", "scr", "com", "pif"];
            if dangerous_extensions.contains(&ext.to_string_lossy().as_ref()) {
                return Err(crate::AgentError::ToolExecution(
                    format!("File extension .{} is not allowed", ext.to_string_lossy())
                ));
            }
        }
        
        Ok(())
    }
    
    fn validate_command(&self, command: &str) -> crate::Result<()> {
        let trimmed = command.trim();
        
        // Check for empty commands
        if trimmed.is_empty() {
            return Err(crate::AgentError::ToolExecution("Empty command not allowed".to_string()));
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
        ];
        
        let command_lower = trimmed.to_lowercase();
        for pattern in &dangerous_patterns {
            if command_lower.contains(pattern) {
                return Err(crate::AgentError::ToolExecution(
                    format!("Dangerous command pattern '{}' not allowed", pattern)
                ));
            }
        }
        
        // Check command length
        if trimmed.len() > 1000 {
            return Err(crate::AgentError::ToolExecution("Command too long".to_string()));
        }
        
        Ok(())
    }
}
```

### 4. Integration with Agent

```rust
// In lib/src/agent.rs - integrate tool execution

impl ClaudeAgent {
    pub fn new(config: AgentConfig) -> crate::Result<(Self, broadcast::Receiver<SessionUpdateNotification>)> {
        let (notification_sender, notification_receiver) = NotificationSender::new();
        let tool_handler = Arc::new(crate::tools::ToolCallHandler::new_with_terminal_manager(
            config.security.to_tool_permissions()
        ));
        
        let agent = Self {
            session_manager: Arc::new(SessionManager::new()),
            claude_client: Arc::new(ClaudeClient::new_with_config(&config.claude)?),
            tool_handler,
            config,
            capabilities: ServerCapabilities {
                streaming: Some(true),
                tools: Some(vec![
                    "fs_read".to_string(),
                    "fs_write".to_string(),
                    "fs_list".to_string(),
                    "terminal_create".to_string(),
                    "terminal_write".to_string(),
                ]),
            },
            notification_sender: Arc::new(notification_sender),
        };
        
        Ok((agent, notification_receiver))
    }
}
```

### 5. Integration Tests

```rust
#[cfg(test)]
mod tool_execution_tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_fs_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_string_lossy();
        
        let handler = create_test_handler();
        
        // Test write
        let write_call = ToolCall {
            id: "write-test".to_string(),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": file_path_str,
                "content": "Hello, World!"
            }),
        };
        
        let write_result = handler.handle_tool_call(write_call).await.unwrap();
        match write_result {
            ToolCallResult::Success(_) => {
                // Expected
            }
            _ => panic!("Write should succeed"),
        }
        
        // Test read
        let read_call = ToolCall {
            id: "read-test".to_string(),
            name: "fs_read".to_string(),
            arguments: json!({
                "path": file_path_str
            }),
        };
        
        let read_result = handler.handle_tool_call(read_call).await.unwrap();
        match read_result {
            ToolCallResult::Success(content) => {
                let text = match &content.content[0] {
                    agent_client_protocol::ContentBlock::Text { text } => text,
                    _ => panic!("Expected text content"),
                };
                assert_eq!(text, "Hello, World!");
            }
            _ => panic!("Read should succeed"),
        }
    }
    
    #[tokio::test]
    async fn test_terminal_operations() {
        let handler = create_test_handler_with_terminal();
        
        // Create terminal
        let create_call = ToolCall {
            id: "create-test".to_string(),
            name: "terminal_create".to_string(),
            arguments: json!({}),
        };
        
        let create_result = handler.handle_tool_call(create_call).await.unwrap();
        let terminal_id = match create_result {
            ToolCallResult::Success(content) => {
                let text = match &content.content[0] {
                    agent_client_protocol::ContentBlock::Text { text } => text,
                    _ => panic!("Expected text content"),
                };
                // Extract terminal ID from response
                text.split_whitespace().last().unwrap().to_string()
            }
            _ => panic!("Terminal creation should succeed"),
        };
        
        // Execute command
        let exec_call = ToolCall {
            id: "exec-test".to_string(),
            name: "terminal_write".to_string(),
            arguments: json!({
                "terminal_id": terminal_id,
                "command": "echo Hello"
            }),
        };
        
        let exec_result = handler.handle_tool_call(exec_call).await.unwrap();
        match exec_result {
            ToolCallResult::Success(_) => {
                // Expected
            }
            _ => panic!("Command execution should succeed"),
        }
    }
}
```

## Files Modified
- `lib/src/tools.rs` - Add actual file system and terminal operations
- `lib/src/agent.rs` - Integrate tool handler with terminal manager
- Add integration tests for tool execution

## Acceptance Criteria
- File system operations (read, write, list) work correctly
- Terminal operations (create, execute commands) work
- Security validation prevents dangerous operations
- Path traversal and other attacks are blocked
- Command validation prevents dangerous commands
- Integration tests verify end-to-end functionality
- Error handling provides useful feedback
- `cargo build` and `cargo test` succeed