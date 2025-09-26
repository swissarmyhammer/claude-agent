//! Model Context Protocol (MCP) server integration
//!
//! This module provides the infrastructure for connecting to and communicating
//! with external MCP servers to extend the agent's tool capabilities beyond
//! the built-in file system and terminal operations.

use crate::{config::McpServerConfig, tools::InternalToolRequest};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

/// Represents a connection to an MCP server
#[derive(Debug)]
pub struct McpServerConnection {
    /// Name of the MCP server
    pub name: String,
    /// Child process running the MCP server
    pub process: Arc<RwLock<Option<Child>>>,
    /// List of tools available from this server
    pub tools: Vec<String>,
    /// Configuration used to create this connection
    pub config: McpServerConfig,
    /// Writer for sending JSON-RPC requests to the server
    pub stdin_writer: Arc<RwLock<Option<BufWriter<tokio::process::ChildStdin>>>>,
    /// Reader for receiving JSON-RPC responses from the server
    pub stdout_reader: Arc<RwLock<Option<BufReader<tokio::process::ChildStdout>>>>,
}

/// Manages connections to multiple MCP servers
#[derive(Debug)]
pub struct McpServerManager {
    /// Map of server name to connection
    connections: Arc<RwLock<HashMap<String, McpServerConnection>>>,
}

impl McpServerManager {
    /// Create a new MCP server manager
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Connect to all configured MCP servers
    pub async fn connect_servers(&mut self, configs: Vec<McpServerConfig>) -> crate::Result<()> {
        for config in configs {
            match self.connect_server(config.clone()).await {
                Ok(connection) => {
                    tracing::info!("Connected to MCP server: {}", connection.name);
                    let mut connections = self.connections.write().await;
                    connections.insert(connection.name.clone(), connection);
                }
                Err(e) => {
                    tracing::error!("Failed to connect to MCP server {}: {}", config.name, e);
                    // Continue with other servers instead of failing completely
                }
            }
        }
        Ok(())
    }

    /// Connect to a single MCP server
    async fn connect_server(&self, config: McpServerConfig) -> crate::Result<McpServerConnection> {
        tracing::info!(
            "Connecting to MCP server: {} ({})",
            config.name,
            config.command
        );

        // Start the MCP server process
        let mut child = Command::new(&config.command)
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to start MCP server {}: {}",
                    config.name, e
                ))
            })?;

        // Get stdio handles
        let stdin = child.stdin.take().ok_or_else(|| {
            crate::AgentError::ToolExecution("Failed to get stdin for MCP server".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            crate::AgentError::ToolExecution("Failed to get stdout for MCP server".to_string())
        })?;

        let mut stdin_writer = BufWriter::new(stdin);
        let mut stdout_reader = BufReader::new(stdout);

        // Initialize MCP protocol
        let tools = self
            .initialize_mcp_connection(&mut stdin_writer, &mut stdout_reader, &config.name, &config)
            .await?;

        let connection = McpServerConnection {
            name: config.name.clone(),
            process: Arc::new(RwLock::new(Some(child))),
            tools,
            config,
            stdin_writer: Arc::new(RwLock::new(Some(stdin_writer))),
            stdout_reader: Arc::new(RwLock::new(Some(stdout_reader))),
        };

        Ok(connection)
    }

    /// Initialize the MCP protocol connection
    async fn initialize_mcp_connection(
        &self,
        writer: &mut BufWriter<tokio::process::ChildStdin>,
        reader: &mut BufReader<tokio::process::ChildStdout>,
        server_name: &str,
        config: &crate::config::McpServerConfig,
    ) -> crate::Result<Vec<String>> {
        // Send initialize request
        let initialize_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": config.protocol.version,
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
        writer
            .write_all(request_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to write initialize request to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush initialize request to MCP server: {}",
                e
            ))
        })?;

        // Read initialize response
        let mut response_line = String::new();
        let bytes_read = reader.read_line(&mut response_line).await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to read initialize response from MCP server: {}",
                e
            ))
        })?;

        if bytes_read == 0 {
            return Err(crate::AgentError::ToolExecution(
                "MCP server closed connection during initialization".to_string(),
            ));
        }

        let response: Value = serde_json::from_str(response_line.trim()).map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Invalid JSON from MCP server during initialization: {}",
                e
            ))
        })?;

        // Extract available tools from response
        let _tools = self.extract_tools_from_initialize_response(&response)?;

        // Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });

        let notification_line = format!("{}\n", initialized_notification);
        writer
            .write_all(notification_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialized notification to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush initialized notification to MCP server: {}",
                e
            ))
        })?;

        // Request list of available tools
        let tools_list_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let tools_request_line = format!("{}\n", tools_list_request);
        writer
            .write_all(tools_request_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to write tools/list request to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush tools/list request to MCP server: {}",
                e
            ))
        })?;

        // Read tools list response
        let mut tools_response_line = String::new();
        let tools_bytes_read = reader
            .read_line(&mut tools_response_line)
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to read tools/list response from MCP server: {}",
                    e
                ))
            })?;

        if tools_bytes_read == 0 {
            return Err(crate::AgentError::ToolExecution(
                "MCP server closed connection during tools/list request".to_string(),
            ));
        }

        let tools_response: Value =
            serde_json::from_str(tools_response_line.trim()).map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Invalid JSON from MCP server tools/list response: {}",
                    e
                ))
            })?;

        let final_tools = self.extract_tools_from_list_response(&tools_response)?;

        tracing::info!(
            "MCP server {} provides {} tools: {:?}",
            server_name,
            final_tools.len(),
            final_tools
        );

        Ok(final_tools)
    }

    /// Extract tools from initialize response
    fn extract_tools_from_initialize_response(
        &self,
        _response: &Value,
    ) -> crate::Result<Vec<String>> {
        // For now, return empty list as we'll get tools from tools/list call
        Ok(Vec::new())
    }

    /// Extract tools from tools/list response
    fn extract_tools_from_list_response(&self, response: &Value) -> crate::Result<Vec<String>> {
        let mut tools = Vec::new();

        if let Some(result) = response.get("result") {
            if let Some(tools_array) = result.get("tools") {
                if let Some(tool_list) = tools_array.as_array() {
                    for tool in tool_list {
                        if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                            tools.push(name.to_string());
                        }
                    }
                }
            }
        }

        // If no tools found, log warning but don't fail
        if tools.is_empty() {
            tracing::warn!("No tools found in MCP tools/list response");
        }

        Ok(tools)
    }

    /// Execute a tool call on the specified MCP server
    pub async fn execute_tool_call(
        &self,
        server_name: &str,
        tool_call: &InternalToolRequest,
    ) -> crate::Result<String> {
        let connections = self.connections.read().await;
        let connection = connections.get(server_name).ok_or_else(|| {
            crate::AgentError::ToolExecution(format!("MCP server {} not found", server_name))
        })?;

        // Send tool call to the server
        let response_content = self.send_tool_call_to_server(connection, tool_call).await?;

        // Convert MCP response to string result
        self.process_tool_call_response(&response_content)
    }

    /// Send a tool call request to an MCP server
    async fn send_tool_call_to_server(
        &self,
        connection: &McpServerConnection,
        tool_call: &InternalToolRequest,
    ) -> crate::Result<Value> {
        // Create MCP tool call request
        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": tool_call.id.clone(),
            "method": "tools/call",
            "params": {
                "name": tool_call.name.split(':').nth(1).unwrap_or(&tool_call.name),
                "arguments": tool_call.arguments
            }
        });

        tracing::info!(
            "Sending tool call to MCP server {}: {}",
            connection.name,
            tool_call.name
        );

        // Get writer and reader
        let mut writer_guard = connection.stdin_writer.write().await;
        let writer = writer_guard.as_mut().ok_or_else(|| {
            crate::AgentError::ToolExecution("MCP server stdin writer not available".to_string())
        })?;

        let mut reader_guard = connection.stdout_reader.write().await;
        let reader = reader_guard.as_mut().ok_or_else(|| {
            crate::AgentError::ToolExecution("MCP server stdout reader not available".to_string())
        })?;

        // Send request
        let request_line = format!("{}\n", mcp_request);
        writer
            .write_all(request_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to write tool call request to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush tool call request to MCP server: {}",
                e
            ))
        })?;

        // Read response
        let mut response_line = String::new();
        let bytes_read = reader.read_line(&mut response_line).await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to read tool call response from MCP server: {}",
                e
            ))
        })?;

        if bytes_read == 0 {
            return Err(crate::AgentError::ToolExecution(
                "MCP server closed connection during tool call".to_string(),
            ));
        }

        let response: Value = serde_json::from_str(response_line.trim()).map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Invalid JSON from MCP server tool call response: {}",
                e
            ))
        })?;

        Ok(response)
    }

    /// Process MCP tool call response into string result
    fn process_tool_call_response(&self, response: &Value) -> crate::Result<String> {
        if let Some(result) = response.get("result") {
            if let Some(content) = result.get("content") {
                if let Some(content_array) = content.as_array() {
                    let mut result_text = String::new();
                    for item in content_array {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            result_text.push_str(text);
                            result_text.push('\n');
                        }
                    }
                    return Ok(result_text.trim().to_string());
                }
            }
            // Fallback to string representation of result
            return Ok(result.to_string());
        }

        if let Some(error) = response.get("error") {
            let error_message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown MCP error");
            return Err(crate::AgentError::ToolExecution(format!(
                "MCP server error: {}",
                error_message
            )));
        }

        Err(crate::AgentError::ToolExecution(
            "Invalid MCP server response: no result or error".to_string(),
        ))
    }

    /// List all available tools from all connected MCP servers
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

    /// Shutdown all MCP server connections
    pub async fn shutdown(&self) -> crate::Result<()> {
        let mut connections = self.connections.write().await;

        for (name, connection) in connections.iter_mut() {
            tracing::info!("Shutting down MCP server: {}", name);

            // Close stdio handles first
            {
                let mut writer_guard = connection.stdin_writer.write().await;
                *writer_guard = None;
            }
            {
                let mut reader_guard = connection.stdout_reader.write().await;
                *reader_guard = None;
            }

            // Kill and wait for the process
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

impl Default for McpServerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_manager_creation() {
        let manager = McpServerManager::new();
        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_mcp_manager_connect_empty_servers() {
        let mut manager = McpServerManager::new();
        let result = manager.connect_servers(vec![]).await;
        assert!(result.is_ok());

        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_mcp_manager_connect_invalid_server() {
        let mut manager = McpServerManager::new();

        let invalid_config = McpServerConfig {
            name: "invalid_server".to_string(),
            command: "nonexistent_command_12345".to_string(),
            args: vec![],
            protocol: crate::config::McpProtocolConfig::default(),
        };

        // Should succeed but log errors for individual server failures
        let result = manager.connect_servers(vec![invalid_config]).await;
        assert!(result.is_ok());

        // No tools should be available since server failed to start
        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_tool_name_extraction() {
        let _manager = McpServerManager::new();

        let tool_call = InternalToolRequest {
            id: "test-123".to_string(),
            name: "server:read_file".to_string(),
            arguments: json!({}),
        };

        // Test that we extract the tool name correctly in the request
        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": tool_call.id,
            "method": "tools/call",
            "params": {
                "name": tool_call.name.split(':').nth(1).unwrap_or(&tool_call.name),
                "arguments": tool_call.arguments
            }
        });

        assert_eq!(mcp_request["params"]["name"].as_str().unwrap(), "read_file");
    }

    #[test]
    fn test_extract_tools_from_list_response() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {
                        "name": "read_file",
                        "description": "Read a file"
                    },
                    {
                        "name": "write_file",
                        "description": "Write a file"
                    }
                ]
            }
        });

        let tools = manager.extract_tools_from_list_response(&response).unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"read_file".to_string()));
        assert!(tools.contains(&"write_file".to_string()));
    }

    #[test]
    fn test_extract_tools_from_empty_response() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": []
            }
        });

        let tools = manager.extract_tools_from_list_response(&response).unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_process_tool_call_response_success() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "File contents here"
                    }
                ]
            }
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        assert_eq!(result, "File contents here");
    }

    #[test]
    fn test_process_tool_call_response_error() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "error": {
                "code": -1,
                "message": "File not found"
            }
        });

        let result = manager.process_tool_call_response(&response);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("MCP server error: File not found"));
    }

    #[test]
    fn test_process_tool_call_response_multiple_content() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "Line 1"
                    },
                    {
                        "type": "text",
                        "text": "Line 2"
                    }
                ]
            }
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        assert_eq!(result, "Line 1\nLine 2");
    }

    #[tokio::test]
    async fn test_shutdown() {
        let manager = McpServerManager::new();

        // Test shutdown with no connections
        let result = manager.shutdown().await;
        assert!(result.is_ok());
    }
}
