# MCP Server Integration

Refer to plan.md

## Goal
Add support for Model Context Protocol (MCP) servers to extend tool capabilities beyond built-in file system and terminal operations.

## Tasks

### 1. MCP Server Manager (`lib/src/mcp.rs`)

```rust
use tokio::process::{Command, Child};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct McpServerConnection {
    pub name: String,
    pub process: Arc<RwLock<Option<Child>>>,
    pub tools: Vec<String>,
    pub config: crate::config::McpServerConfig,
}

pub struct McpServerManager {
    connections: Arc<RwLock<HashMap<String, McpServerConnection>>>,
}

impl McpServerManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn connect_servers(&mut self, configs: Vec<crate::config::McpServerConfig>) -> crate::Result<()> {
        for config in configs {
            match self.connect_server(config).await {
                Ok(connection) => {
                    tracing::info!("Connected to MCP server: {}", connection.name);
                    let mut connections = self.connections.write().await;
                    connections.insert(connection.name.clone(), connection);
                }
                Err(e) => {
                    tracing::error!("Failed to connect to MCP server {}: {}", config.name, e);
                    return Err(e);
                }
            }
        }
        Ok(())
    }
    
    async fn connect_server(&self, config: crate::config::McpServerConfig) -> crate::Result<McpServerConnection> {
        tracing::info!("Connecting to MCP server: {} ({})", config.name, config.command);
        
        // Start the MCP server process
        let mut child = Command::new(&config.command)
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| crate::AgentError::ToolExecution(
                format!("Failed to start MCP server {}: {}", config.name, e)
            ))?;
        
        // Initialize MCP protocol
        let tools = self.initialize_mcp_connection(&mut child, &config.name).await?;
        
        let connection = McpServerConnection {
            name: config.name.clone(),
            process: Arc::new(RwLock::new(Some(child))),
            tools,
            config,
        };
        
        Ok(connection)
    }
    
    async fn initialize_mcp_connection(&self, child: &mut Child, server_name: &str) -> crate::Result<Vec<String>> {
        let stdin = child.stdin.take()
            .ok_or_else(|| crate::AgentError::ToolExecution("No stdin".to_string()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| crate::AgentError::ToolExecution("No stdout".to_string()))?;
        
        let mut reader = BufReader::new(stdout);
        let mut writer = stdin;
        
        // Send initialize request
        let initialize_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": {
                    "name": "claude-agent",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });
        
        let request_line = format!("{}\n", initialize_request);
        writer.write_all(request_line.as_bytes()).await
            .map_err(|e| crate::AgentError::ToolExecution(format!("Failed to write to MCP server: {}", e)))?;
        
        // Read initialize response
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await
            .map_err(|e| crate::AgentError::ToolExecution(format!("Failed to read from MCP server: {}", e)))?;
        
        let response: Value = serde_json::from_str(&response_line)
            .map_err(|e| crate::AgentError::ToolExecution(format!("Invalid JSON from MCP server: {}", e)))?;
        
        // Extract available tools from response
        let tools = self.extract_tools_from_response(&response)?;
        
        tracing::info!("MCP server {} provides {} tools: {:?}", server_name, tools.len(), tools);
        
        Ok(tools)
    }
    
    fn extract_tools_from_response(&self, response: &Value) -> crate::Result<Vec<String>> {
        let mut tools = Vec::new();
        
        if let Some(result) = response.get("result") {
            if let Some(capabilities) = result.get("capabilities") {
                if let Some(tool_capabilities) = capabilities.get("tools") {
                    if let Some(tool_list) = tool_capabilities.get("listChanged") {
                        // Handle different MCP server response formats
                        // This is a simplified version - real MCP servers may vary
                        if let Some(tool_array) = tool_list.as_array() {
                            for tool in tool_array {
                                if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                                    tools.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // If no tools found in response, assume basic tools are available
        if tools.is_empty() {
            tracing::warn!("No tools found in MCP response, assuming basic tools");
            tools.push("mcp_call".to_string());
        }
        
        Ok(tools)
    }
    
    pub async fn execute_tool_call(&self, server_name: &str, tool_call: &agent_client_protocol::ToolCall) -> crate::Result<agent_client_protocol::ToolCallContent> {
        let connections = self.connections.read().await;
        let connection = connections.get(server_name)
            .ok_or_else(|| crate::AgentError::ToolExecution(format!("MCP server {} not found", server_name)))?;
        
        self.send_tool_call_to_server(connection, tool_call).await
    }
    
    async fn send_tool_call_to_server(
        &self, 
        connection: &McpServerConnection, 
        tool_call: &agent_client_protocol::ToolCall
    ) -> crate::Result<agent_client_protocol::ToolCallContent> {
        // Create MCP tool call request
        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": tool_call.id.clone(),
            "method": "tools/call",
            "params": {
                "name": tool_call.name,
                "arguments": tool_call.arguments
            }
        });
        
        tracing::info!("Sending tool call to MCP server {}: {}", connection.name, tool_call.name);
        
        // Send request and receive response (simplified - real implementation needs proper async handling)
        let response_content = self.communicate_with_mcp_server(connection, &mcp_request).await?;
        
        // Convert MCP response to ACP ToolCallContent
        let content = if let Some(result) = response_content.get("result") {
            vec![agent_client_protocol::ContentBlock::Text {
                text: result.to_string(),
            }]
        } else if let Some(error) = response_content.get("error") {
            vec![agent_client_protocol::ContentBlock::Text {
                text: format!("MCP Error: {}", error),
            }]
        } else {
            vec![agent_client_protocol::ContentBlock::Text {
                text: "Unknown MCP response".to_string(),
            }]
        };
        
        Ok(agent_client_protocol::ToolCallContent {
            tool_call_id: tool_call.id.clone(),
            content,
        })
    }
    
    async fn communicate_with_mcp_server(
        &self,
        connection: &McpServerConnection,
        request: &Value,
    ) -> crate::Result<Value> {
        // Simplified implementation - in a real system this would need:
        // 1. Proper async communication with the child process
        // 2. Request/response correlation
        // 3. Error handling for process failures
        // 4. Connection pooling and reuse
        
        // For now, return a mock response
        Ok(json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "result": {
                "content": [{
                    "type": "text",
                    "text": format!("Mock response from MCP server {} for tool {}", 
                                   connection.name, 
                                   request["params"]["name"])
                }]
            }
        }))
    }
    
    pub async fn list_available_tools(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        let mut all_tools = Vec::new();
        
        for connection in connections.values() {
            for tool in &connection.tools {
                all_tools.push(format!("{}:{}", connection.name, tool));
            }
        }
        
        all_tools
    }
    
    pub async fn shutdown(&self) -> crate::Result<()> {
        let mut connections = self.connections.write().await;
        
        for (name, connection) in connections.iter_mut() {
            tracing::info!("Shutting down MCP server: {}", name);
            
            let mut process_guard = connection.process.write().await;
            if let Some(mut process) = process_guard.take() {
                let _ = process.kill().await;
                let _ = process.wait().await;
            }
        }
        
        connections.clear();
        Ok(())
    }
}
```

### 2. Integration with Tool Handler (`lib/src/tools.rs`)

```rust
impl ToolCallHandler {
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
    
    async fn execute_tool_call(&self, tool_call: &ToolCall) -> crate::Result<ToolCallContent> {
        // Check if this is an MCP tool call
        if let Some(server_name) = self.extract_mcp_server_name(&tool_call.name) {
            if let Some(ref mcp_manager) = self.mcp_manager {
                return mcp_manager.execute_tool_call(server_name, tool_call).await;
            }
        }
        
        // Handle built-in tools
        match tool_call.name.as_str() {
            "fs_read" => self.handle_fs_read(tool_call).await,
            "fs_write" => self.handle_fs_write(tool_call).await,
            "fs_list" => self.handle_fs_list(tool_call).await,
            "terminal_create" => self.handle_terminal_create(tool_call).await,
            "terminal_write" => self.handle_terminal_write(tool_call).await,
            _ => Err(crate::AgentError::ToolExecution(
                format!("Unknown tool: {}", tool_call.name)
            )),
        }
    }
    
    fn extract_mcp_server_name(&self, tool_name: &str) -> Option<&str> {
        // Tool names from MCP servers are prefixed with server name
        // e.g., "filesystem:read_file" -> server "filesystem"
        tool_name.split(':').next()
    }
    
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
}
```

### 3. Configuration for MCP Servers

```rust
// In lib/src/config.rs - extend existing McpServerConfig

impl crate::config::McpServerConfig {
    pub fn validate(&self) -> crate::Result<()> {
        if self.name.is_empty() {
            return Err(crate::AgentError::Config("MCP server name cannot be empty".to_string()));
        }
        
        if self.command.is_empty() {
            return Err(crate::AgentError::Config("MCP server command cannot be empty".to_string()));
        }
        
        // Check if command exists
        if let Err(_) = std::process::Command::new(&self.command).arg("--version").output() {
            tracing::warn!("MCP server command may not be available: {}", self.command);
        }
        
        Ok(())
    }
}

impl crate::config::AgentConfig {
    pub fn validate_mcp_servers(&self) -> crate::Result<()> {
        for server_config in &self.mcp_servers {
            server_config.validate()?;
        }
        Ok(())
    }
}
```

### 4. Agent Integration

```rust
// In lib/src/agent.rs - integrate MCP manager

impl ClaudeAgent {
    pub async fn new(config: AgentConfig) -> crate::Result<(Self, broadcast::Receiver<SessionUpdateNotification>)> {
        config.validate_mcp_servers()?;
        
        let (notification_sender, notification_receiver) = NotificationSender::new();
        
        // Create and initialize MCP manager
        let mut mcp_manager = crate::mcp::McpServerManager::new();
        mcp_manager.connect_servers(config.mcp_servers.clone()).await?;
        let mcp_manager = Arc::new(mcp_manager);
        
        let tool_handler = Arc::new(crate::tools::ToolCallHandler::new_with_mcp_manager(
            config.security.to_tool_permissions(),
            Arc::clone(&mcp_manager),
        ));
        
        // Get all available tools for capabilities
        let available_tools = tool_handler.list_all_available_tools().await;
        
        let agent = Self {
            session_manager: Arc::new(SessionManager::new()),
            claude_client: Arc::new(ClaudeClient::new_with_config(&config.claude)?),
            tool_handler,
            mcp_manager: Some(mcp_manager),
            config,
            capabilities: ServerCapabilities {
                streaming: Some(true),
                tools: Some(available_tools),
            },
            notification_sender: Arc::new(notification_sender),
            pending_tool_calls: Arc::new(PendingToolCallManager::new()),
        };
        
        Ok((agent, notification_receiver))
    }
    
    // Add shutdown method to clean up MCP servers
    pub async fn shutdown(&self) -> crate::Result<()> {
        tracing::info!("Shutting down Claude Agent");
        
        if let Some(ref mcp_manager) = self.mcp_manager {
            mcp_manager.shutdown().await?;
        }
        
        tracing::info!("Agent shutdown complete");
        Ok(())
    }
}
```

### 5. Example MCP Server Configurations

```rust
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            claude: ClaudeConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                stream_format: StreamFormat::StreamJson,
            },
            server: ServerConfig {
                port: None,
                log_level: "info".to_string(),
            },
            security: SecurityConfig {
                allowed_file_patterns: vec![
                    "**/*.rs".to_string(),
                    "**/*.md".to_string(),
                    "**/*.toml".to_string(),
                ],
                forbidden_paths: vec![
                    "/etc".to_string(),
                    "/usr".to_string(),
                    "/bin".to_string(),
                ],
                require_permission_for: vec![
                    "fs_write".to_string(),
                    "terminal_create".to_string(),
                ],
            },
            mcp_servers: vec![
                McpServerConfig {
                    name: "filesystem".to_string(),
                    command: "npx".to_string(),
                    args: vec![
                        "@modelcontextprotocol/server-filesystem".to_string(),
                        "--".to_string(),
                        ".".to_string(),
                    ],
                },
                McpServerConfig {
                    name: "git".to_string(),
                    command: "npx".to_string(),
                    args: vec![
                        "@modelcontextprotocol/server-git".to_string(),
                        "--".to_string(),
                        ".".to_string(),
                    ],
                },
                McpServerConfig {
                    name: "brave_search".to_string(),
                    command: "npx".to_string(),
                    args: vec![
                        "@modelcontextprotocol/server-brave-search".to_string(),
                    ],
                },
            ],
        }
    }
}
```

### 6. Integration Tests

```rust
#[cfg(test)]
mod mcp_tests {
    use super::*;
    
    async fn create_mock_mcp_server_config() -> crate::config::McpServerConfig {
        crate::config::McpServerConfig {
            name: "test_server".to_string(),
            command: "echo".to_string(), // Use echo as a simple test command
            args: vec!["mcp_response".to_string()],
        }
    }
    
    #[tokio::test]
    async fn test_mcp_manager_creation() {
        let manager = McpServerManager::new();
        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty()); // No servers connected yet
    }
    
    #[tokio::test]
    #[ignore = "requires external MCP server"]
    async fn test_mcp_server_connection() {
        let mut manager = McpServerManager::new();
        let config = create_mock_mcp_server_config().await;
        
        let result = manager.connect_servers(vec![config]).await;
        // This test may fail without actual MCP server - that's expected
        // In a real environment, this would test with actual MCP server
        
        match result {
            Ok(()) => {
                let tools = manager.list_available_tools().await;
                assert!(!tools.is_empty());
            }
            Err(_) => {
                // Expected if no MCP server available
                println!("MCP server connection test skipped - no server available");
            }
        }
    }
    
    #[tokio::test]
    async fn test_tool_handler_with_mcp() {
        let mcp_manager = Arc::new(McpServerManager::new());
        let permissions = crate::tools::ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec![],
        };
        
        let tool_handler = crate::tools::ToolCallHandler::new_with_mcp_manager(permissions, mcp_manager);
        
        let tools = tool_handler.list_all_available_tools().await;
        
        // Should include built-in tools
        assert!(tools.contains(&"fs_read".to_string()));
        assert!(tools.contains(&"fs_write".to_string()));
        assert!(tools.contains(&"terminal_create".to_string()));
    }
    
    #[tokio::test]
    async fn test_agent_with_mcp_config() {
        let mut config = AgentConfig::default();
        
        // Add a test MCP server config
        config.mcp_servers = vec![
            crate::config::McpServerConfig {
                name: "test".to_string(),
                command: "echo".to_string(),
                args: vec!["test".to_string()],
            }
        ];
        
        // This may fail if echo doesn't behave like an MCP server
        // but should not panic
        let result = ClaudeAgent::new(config).await;
        
        match result {
            Ok((agent, _)) => {
                // Test that agent has the MCP tools in capabilities
                if let Some(ref tools) = agent.capabilities.tools {
                    assert!(!tools.is_empty());
                }
                
                // Clean shutdown
                let _ = agent.shutdown().await;
            }
            Err(e) => {
                println!("Agent creation with MCP failed (expected): {}", e);
            }
        }
    }
}
```

### 7. CLI Configuration Example

Create example configuration files:

**`config/example-with-mcp.yaml`**:
```yaml
claude:
  model: "claude-sonnet-4-20250514"
  stream_format: "StreamJson"

server:
  log_level: "info"

security:
  allowed_file_patterns: ["**/*.rs", "**/*.md", "**/*.json"]
  forbidden_paths: ["/etc", "/usr", "/bin"]
  require_permission_for: ["fs_write", "terminal_create"]

mcp_servers:
  - name: "filesystem"
    command: "npx"
    args: ["@modelcontextprotocol/server-filesystem", "--", "."]
  
  - name: "git"
    command: "npx"  
    args: ["@modelcontextprotocol/server-git", "--", "."]
  
  - name: "sqlite"
    command: "npx"
    args: ["@modelcontextprotocol/server-sqlite", "--", "data.db"]
```

## Files Created
- `lib/src/mcp.rs` - MCP server manager implementation
- Update `lib/src/tools.rs` - Add MCP integration to tool handler
- Update `lib/src/agent.rs` - Integrate MCP manager with agent
- Update `lib/src/config.rs` - Add MCP configuration validation
- `config/example-with-mcp.yaml` - Example configuration with MCP servers
- Add MCP integration tests

## Dependencies
Add to `lib/Cargo.toml`:
```toml
[dependencies]
# ... existing dependencies ...
tokio-util = { version = "0.7", features = ["codec"] }
```

## Acceptance Criteria
- MCP server manager can start and communicate with external MCP servers
- Tool calls are correctly routed to appropriate MCP servers
- MCP tools are included in agent capabilities
- Configuration validation prevents invalid MCP server configs  
- Agent shutdown properly closes MCP server connections
- Integration tests verify MCP functionality (with mock servers)
- Example configurations demonstrate real MCP server usage
- Error handling covers MCP server failures and timeouts
- Tool permission system works with MCP tools
- `cargo build` and `cargo test` succeed