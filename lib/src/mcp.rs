//! Model Context Protocol (MCP) server integration
//!
//! This module provides the infrastructure for connecting to and communicating
//! with external MCP servers to extend the agent's tool capabilities beyond
//! the built-in file system and terminal operations.

use crate::{config::McpServerConfig, error::McpError, tools::InternalToolRequest};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};

/// Transport-specific connection details
#[derive(Debug)]
pub enum TransportConnection {
    /// Stdio transport using child process
    Stdio {
        process: Arc<RwLock<Option<Child>>>,
        stdin_writer: Arc<RwLock<Option<BufWriter<tokio::process::ChildStdin>>>>,
        stdout_reader: Arc<RwLock<Option<BufReader<tokio::process::ChildStdout>>>>,
    },
    /// HTTP transport using reqwest client
    Http {
        client: Arc<Client>,
        url: String,
        headers: Vec<crate::config::HttpHeader>,
        session_id: Arc<RwLock<Option<String>>>,
    },
    /// SSE transport using WebSocket connection
    Sse {
        url: String,
        headers: Vec<crate::config::HttpHeader>,
        message_tx: Arc<RwLock<Option<mpsc::UnboundedSender<String>>>>,
        response_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<String>>>>,
    },
}

/// Represents a connection to an MCP server
#[derive(Debug)]
pub struct McpServerConnection {
    /// Name of the MCP server
    pub name: String,
    /// List of tools available from this server
    pub tools: Vec<String>,
    /// Configuration used to create this connection
    pub config: McpServerConfig,
    /// Transport-specific connection details
    pub transport: TransportConnection,
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
                    tracing::error!("Failed to connect to MCP server {}: {}", config.name(), e);
                    // Continue with other servers instead of failing completely
                }
            }
        }
        Ok(())
    }

    /// Connect to a single MCP server
    async fn connect_server(&self, config: McpServerConfig) -> crate::Result<McpServerConnection> {
        // Only stdio transport is currently implemented in the connection logic
        match &config {
            McpServerConfig::Stdio(stdio_config) => {
                tracing::info!(
                    "Connecting to MCP server: {} ({})",
                    stdio_config.name,
                    stdio_config.command
                );

                // Start the MCP server process with environment variables
                let mut command = Command::new(&stdio_config.command);
                command.args(&stdio_config.args);

                // Set working directory if provided
                if let Some(cwd) = &stdio_config.cwd {
                    command.current_dir(cwd);
                }

                // Set environment variables
                for env_var in &stdio_config.env {
                    command.env(&env_var.name, &env_var.value);
                }

                let mut child = command
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| McpError::ProcessSpawnFailed(stdio_config.name.clone(), e))?;

                // Get stdio handles
                let stdin = child.stdin.take().ok_or(McpError::StdinNotAvailable)?;
                let stdout = child.stdout.take().ok_or(McpError::StdoutNotAvailable)?;

                let mut stdin_writer = BufWriter::new(stdin);
                let mut stdout_reader = BufReader::new(stdout);

                // Initialize MCP protocol
                let tools = self
                    .initialize_mcp_connection(
                        &mut stdin_writer,
                        &mut stdout_reader,
                        &stdio_config.name,
                        stdio_config,
                    )
                    .await?;

                let transport = TransportConnection::Stdio {
                    process: Arc::new(RwLock::new(Some(child))),
                    stdin_writer: Arc::new(RwLock::new(Some(stdin_writer))),
                    stdout_reader: Arc::new(RwLock::new(Some(stdout_reader))),
                };

                let connection = McpServerConnection {
                    name: stdio_config.name.clone(),
                    tools,
                    config,
                    transport,
                };

                Ok(connection)
            }
            McpServerConfig::Http(http_config) => {
                tracing::info!(
                    "Connecting to HTTP MCP server: {} ({})",
                    http_config.name,
                    http_config.url
                );

                // Create HTTP client with headers
                let client_builder = Client::builder();
                let mut headers = reqwest::header::HeaderMap::new();

                for header in &http_config.headers {
                    if let (Ok(name), Ok(value)) = (
                        reqwest::header::HeaderName::from_bytes(header.name.as_bytes()),
                        reqwest::header::HeaderValue::from_str(&header.value),
                    ) {
                        headers.insert(name, value);
                    }
                }

                let client = client_builder
                    .default_headers(headers)
                    .build()
                    .map_err(|e| {
                        crate::AgentError::ToolExecution(format!(
                            "Failed to create HTTP client for MCP server {}: {}",
                            http_config.name, e
                        ))
                    })?;

                // Initialize MCP connection via HTTP
                let session_id = Arc::new(RwLock::new(None));
                let tools = self
                    .initialize_http_mcp_connection(&client, http_config, session_id.clone())
                    .await?;

                let transport = TransportConnection::Http {
                    client: Arc::new(client),
                    url: http_config.url.clone(),
                    headers: http_config.headers.clone(),
                    session_id,
                };

                let connection = McpServerConnection {
                    name: http_config.name.clone(),
                    tools,
                    config,
                    transport,
                };

                Ok(connection)
            }
            McpServerConfig::Sse(sse_config) => {
                tracing::info!(
                    "Connecting to SSE MCP server: {} ({})",
                    sse_config.name,
                    sse_config.url
                );

                // Create SSE connection channels
                let (message_tx, _message_rx) = mpsc::unbounded_channel();
                let (response_tx, response_rx) = mpsc::unbounded_channel();

                // Initialize SSE connection
                let tools = self
                    .initialize_sse_mcp_connection(sse_config, response_tx)
                    .await?;

                let transport = TransportConnection::Sse {
                    url: sse_config.url.clone(),
                    headers: sse_config.headers.clone(),
                    message_tx: Arc::new(RwLock::new(Some(message_tx))),
                    response_rx: Arc::new(RwLock::new(Some(response_rx))),
                };

                let connection = McpServerConnection {
                    name: sse_config.name.clone(),
                    tools,
                    config,
                    transport,
                };

                Ok(connection)
            }
        }
    }

    /// Initialize the MCP protocol connection
    async fn initialize_mcp_connection(
        &self,
        writer: &mut BufWriter<tokio::process::ChildStdin>,
        reader: &mut BufReader<tokio::process::ChildStdout>,
        server_name: &str,
        _config: &crate::config::StdioTransport,
    ) -> crate::Result<Vec<String>> {
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
        let bytes_read = reader
            .read_line(&mut response_line)
            .await
            .map_err(McpError::IoError)?;

        if bytes_read == 0 {
            return Err(McpError::ConnectionClosed.into());
        }

        let response: Value =
            serde_json::from_str(response_line.trim()).map_err(McpError::SerializationFailed)?;

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

    /// Initialize HTTP MCP connection using the MCP Streamable HTTP transport protocol.
    ///
    /// Implements the three-step initialization handshake:
    /// 1. Send initialize request and parse response (JSON or SSE)
    /// 2. Send initialized notification (expects HTTP 202)
    /// 3. Request tools list and extract tool names
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `config` - HTTP transport configuration including URL and headers
    /// * `session_id` - Arc-wrapped session ID storage for subsequent requests
    ///
    /// # Returns
    /// List of available tool names from the MCP server
    ///
    /// # Errors
    /// Returns error if:
    /// - Connection fails
    /// - Server returns non-success status
    /// - Response parsing fails
    /// - Protocol negotiation fails
    async fn initialize_http_mcp_connection(
        &self,
        client: &Client,
        config: &crate::config::HttpTransport,
        session_id: Arc<RwLock<Option<String>>>,
    ) -> crate::Result<Vec<String>> {
        tracing::info!("Initializing HTTP MCP protocol for {}", config.name);

        // Step 1: Send initialize request via HTTP POST
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

        let response = client
            .post(&config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .json(&initialize_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialize request to HTTP MCP server: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            return Err(crate::AgentError::ToolExecution(format!(
                "Initialize request failed with status: {}",
                response.status()
            )));
        }

        // Extract session ID if present
        if let Some(session_id_header) = response.headers().get("Mcp-Session-Id") {
            if let Ok(session_id_str) = session_id_header.to_str() {
                let mut session_id_write = session_id.write().await;
                *session_id_write = Some(session_id_str.to_string());
                tracing::debug!("Stored session ID: {}", session_id_str);
            }
        }

        // Parse response body - handle both JSON and SSE formats
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");

        let initialize_response: Value = if content_type.contains("text/event-stream") {
            // Handle SSE response format
            let body = response.text().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to read SSE response from HTTP MCP server: {}",
                    e
                ))
            })?;

            // Parse SSE format - look for data: lines
            let mut json_data = String::new();
            for line in body.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    json_data = data.to_string();
                    break;
                }
            }

            if json_data.is_empty() {
                return Err(crate::AgentError::ToolExecution(
                    "No data in SSE response from HTTP MCP server".to_string(),
                ));
            }

            serde_json::from_str(&json_data).map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Invalid JSON in SSE response from HTTP MCP server: {}",
                    e
                ))
            })?
        } else {
            // Handle JSON response format
            response.json().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to parse initialize response from HTTP MCP server: {}",
                    e
                ))
            })?
        };

        // Validate initialize response
        if let Some(error) = initialize_response.get("error") {
            return Err(crate::AgentError::ToolExecution(format!(
                "HTTP MCP server returned error: {}",
                error
            )));
        }

        // Step 2: Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });

        let mut notify_request = client
            .post(&config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");

        // Include session ID if present
        if let Some(session_id_value) = session_id.read().await.as_ref() {
            notify_request = notify_request.header("Mcp-Session-Id", session_id_value);
        }

        let notify_response = notify_request
            .json(&initialized_notification)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialized notification to HTTP MCP server: {}",
                    e
                ))
            })?;

        // Expect 202 Accepted for notification
        if notify_response.status() != reqwest::StatusCode::ACCEPTED {
            tracing::warn!(
                "Initialized notification returned status: {} (expected 202 Accepted)",
                notify_response.status()
            );
        }

        // Step 3: Request list of available tools
        let tools_list_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let mut tools_request = client
            .post(&config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");

        // Include session ID if present
        if let Some(session_id_value) = session_id.read().await.as_ref() {
            tools_request = tools_request.header("Mcp-Session-Id", session_id_value);
        }

        let tools_response = tools_request
            .json(&tools_list_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send tools/list request to HTTP MCP server: {}",
                    e
                ))
            })?;

        if !tools_response.status().is_success() {
            return Err(crate::AgentError::ToolExecution(format!(
                "Tools list request failed with status: {}",
                tools_response.status()
            )));
        }

        // Parse tools response - handle both JSON and SSE formats
        let tools_content_type = tools_response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");

        let tools_response_json: Value = if tools_content_type.contains("text/event-stream") {
            // Handle SSE response format
            let body = tools_response.text().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to read tools SSE response from HTTP MCP server: {}",
                    e
                ))
            })?;

            // Parse SSE format - look for data: lines
            let mut json_data = String::new();
            for line in body.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    json_data = data.to_string();
                    break;
                }
            }

            if json_data.is_empty() {
                return Err(crate::AgentError::ToolExecution(
                    "No data in tools SSE response from HTTP MCP server".to_string(),
                ));
            }

            serde_json::from_str(&json_data).map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Invalid JSON in tools SSE response from HTTP MCP server: {}",
                    e
                ))
            })?
        } else {
            // Handle JSON response format
            tools_response.json().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to parse tools/list response from HTTP MCP server: {}",
                    e
                ))
            })?
        };

        let final_tools = self.extract_tools_from_list_response(&tools_response_json)?;

        tracing::info!(
            "HTTP MCP server {} provides {} tools: {:?}",
            config.name,
            final_tools.len(),
            final_tools
        );

        Ok(final_tools)
    }

    /// Initialize SSE MCP connection
    async fn initialize_sse_mcp_connection(
        &self,
        config: &crate::config::SseTransport,
        _response_tx: mpsc::UnboundedSender<String>,
    ) -> crate::Result<Vec<String>> {
        // For now, return empty tools list as SSE implementation is complex
        // This is a placeholder for the full SSE implementation
        tracing::warn!(
            "SSE MCP server {} connection initialized but not fully implemented",
            config.name
        );

        Ok(vec![])
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
            McpError::InvalidConfiguration(format!("MCP server '{}' not found", server_name))
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

        match &connection.transport {
            TransportConnection::Stdio {
                stdin_writer,
                stdout_reader,
                ..
            } => {
                // Get writer and reader
                let mut writer_guard = stdin_writer.write().await;
                let writer = writer_guard.as_mut().ok_or(McpError::StdinNotAvailable)?;

                let mut reader_guard = stdout_reader.write().await;
                let reader = reader_guard.as_mut().ok_or(McpError::StdoutNotAvailable)?;

                // Send request
                let request_line = format!("{}\n", mcp_request);
                writer
                    .write_all(request_line.as_bytes())
                    .await
                    .map_err(McpError::IoError)?;
                writer.flush().await.map_err(McpError::IoError)?;

                // Read response
                let mut response_line = String::new();
                let bytes_read = reader
                    .read_line(&mut response_line)
                    .await
                    .map_err(McpError::IoError)?;

                if bytes_read == 0 {
                    return Err(McpError::ConnectionClosed.into());
                }

                let response: Value = serde_json::from_str(response_line.trim())
                    .map_err(McpError::SerializationFailed)?;

                Ok(response)
            }
            TransportConnection::Http {
                client,
                url,
                session_id,
                ..
            } => {
                // Send HTTP request with session ID if available
                let mut request = client
                    .post(url)
                    .header("Accept", "application/json, text/event-stream")
                    .header("Content-Type", "application/json");

                // Include session ID if present
                if let Some(session_id_value) = session_id.read().await.as_ref() {
                    request = request.header("Mcp-Session-Id", session_id_value);
                }

                let response = request.json(&mcp_request).send().await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to send HTTP tool call request to MCP server: {}",
                        e
                    ))
                })?;

                let response_json: Value = response.json().await.map_err(|e| {
                    McpError::ProtocolError(format!(
                        "Failed to parse HTTP tool call response from MCP server: {}",
                        e
                    ))
                })?;

                Ok(response_json)
            }
            TransportConnection::Sse { .. } => {
                // SSE transport not fully implemented yet
                Err(crate::AgentError::ToolExecution(
                    "SSE transport tool calls not yet implemented".to_string(),
                ))
            }
        }
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
            return Err(McpError::ServerError(error.clone()).into());
        }

        Err(McpError::MissingResult.into())
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

            match &connection.transport {
                TransportConnection::Stdio {
                    process,
                    stdin_writer,
                    stdout_reader,
                } => {
                    // Close stdio handles first
                    {
                        let mut writer_guard = stdin_writer.write().await;
                        *writer_guard = None;
                    }
                    {
                        let mut reader_guard = stdout_reader.write().await;
                        *reader_guard = None;
                    }

                    // Kill and wait for the process
                    let mut process_guard = process.write().await;
                    if let Some(mut proc) = process_guard.take() {
                        let _ = proc.kill().await;
                        let _ = proc.wait().await;
                    }
                }
                TransportConnection::Http { .. } => {
                    // HTTP connections don't need explicit cleanup
                    tracing::debug!("HTTP MCP server connection closed: {}", name);
                }
                TransportConnection::Sse {
                    message_tx,
                    response_rx,
                    ..
                } => {
                    // Close SSE channels
                    {
                        let mut tx_guard = message_tx.write().await;
                        *tx_guard = None;
                    }
                    {
                        let mut rx_guard = response_rx.write().await;
                        *rx_guard = None;
                    }
                    tracing::debug!("SSE MCP server connection closed: {}", name);
                }
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

        let invalid_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "invalid_server".to_string(),
            command: "nonexistent_command_12345".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });

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
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("MCP server error:"));
        assert!(error_message.contains("File not found"));
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

    #[tokio::test]
    async fn test_stdio_transport_connection_invalid_command() {
        let mut manager = McpServerManager::new();

        let stdio_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "invalid_server".to_string(),
            command: "nonexistent_command_12345".to_string(),
            args: vec!["--stdio".to_string()],
            env: vec![crate::config::EnvVariable {
                name: "TEST_VAR".to_string(),
                value: "test_value".to_string(),
            }],
            cwd: None,
        });

        // Should fail gracefully and log errors
        let result = manager.connect_servers(vec![stdio_config]).await;
        assert!(result.is_ok()); // Manager should continue despite individual failures

        // No tools should be available since server failed to start
        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_http_transport_configuration() {
        let _manager = McpServerManager::new();

        let http_config = crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test-http-server".to_string(),
            url: "https://api.example.com/mcp".to_string(),
            headers: vec![
                crate::config::HttpHeader {
                    name: "Authorization".to_string(),
                    value: "Bearer token123".to_string(),
                },
                crate::config::HttpHeader {
                    name: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                },
            ],
        };

        // Test validation
        assert!(http_config.validate().is_ok());

        let mcp_config = McpServerConfig::Http(http_config);
        assert_eq!(mcp_config.name(), "test-http-server");
        assert_eq!(mcp_config.transport_type(), "http");
    }

    #[tokio::test]
    async fn test_sse_transport_configuration() {
        let _manager = McpServerManager::new();

        let sse_config = crate::config::SseTransport {
            transport_type: "sse".to_string(),
            name: "test-sse-server".to_string(),
            url: "https://events.example.com/mcp".to_string(),
            headers: vec![crate::config::HttpHeader {
                name: "X-API-Key".to_string(),
                value: "apikey456".to_string(),
            }],
        };

        // Test validation
        assert!(sse_config.validate().is_ok());

        let mcp_config = McpServerConfig::Sse(sse_config);
        assert_eq!(mcp_config.name(), "test-sse-server");
        assert_eq!(mcp_config.transport_type(), "sse");
    }

    #[test]
    fn test_transport_type_detection() {
        let stdio_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "stdio-test".to_string(),
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            env: vec![],
            cwd: None,
        });

        let http_config = McpServerConfig::Http(crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "http-test".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
        });

        let sse_config = McpServerConfig::Sse(crate::config::SseTransport {
            transport_type: "sse".to_string(),
            name: "sse-test".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
        });

        assert_eq!(stdio_config.transport_type(), "stdio");
        assert_eq!(http_config.transport_type(), "http");
        assert_eq!(sse_config.transport_type(), "sse");

        assert_eq!(stdio_config.name(), "stdio-test");
        assert_eq!(http_config.name(), "http-test");
        assert_eq!(sse_config.name(), "sse-test");
    }

    #[test]
    fn test_transport_validation_error_cases() {
        // Test stdio with empty command
        let invalid_stdio = crate::config::StdioTransport {
            name: "test".to_string(),
            command: String::new(),
            args: vec![],
            env: vec![],
            cwd: None,
        };
        assert!(invalid_stdio.validate().is_err());

        // Test HTTP with invalid URL
        let invalid_http = crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test".to_string(),
            url: "ftp://invalid-protocol.com".to_string(),
            headers: vec![],
        };
        assert!(invalid_http.validate().is_err());

        // Test SSE with empty name
        let invalid_sse = crate::config::SseTransport {
            transport_type: "sse".to_string(),
            name: String::new(),
            url: "https://example.com".to_string(),
            headers: vec![],
        };
        assert!(invalid_sse.validate().is_err());

        // Test env var with empty name
        let invalid_stdio_env = crate::config::StdioTransport {
            name: "test".to_string(),
            command: "/bin/echo".to_string(),
            args: vec![],
            env: vec![crate::config::EnvVariable {
                name: String::new(),
                value: "value".to_string(),
            }],
            cwd: None,
        };
        assert!(invalid_stdio_env.validate().is_err());

        // Test HTTP header with empty name
        let invalid_http_header = crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![crate::config::HttpHeader {
                name: String::new(),
                value: "value".to_string(),
            }],
        };
        assert!(invalid_http_header.validate().is_err());
    }

    #[test]
    fn test_env_variable_and_http_header_equality() {
        let env1 = crate::config::EnvVariable {
            name: "API_KEY".to_string(),
            value: "secret123".to_string(),
        };
        let env2 = crate::config::EnvVariable {
            name: "API_KEY".to_string(),
            value: "secret123".to_string(),
        };
        let env3 = crate::config::EnvVariable {
            name: "API_KEY".to_string(),
            value: "different_secret".to_string(),
        };

        assert_eq!(env1, env2);
        assert_ne!(env1, env3);

        let header1 = crate::config::HttpHeader {
            name: "Authorization".to_string(),
            value: "Bearer token".to_string(),
        };
        let header2 = crate::config::HttpHeader {
            name: "Authorization".to_string(),
            value: "Bearer token".to_string(),
        };
        let header3 = crate::config::HttpHeader {
            name: "Authorization".to_string(),
            value: "Bearer different_token".to_string(),
        };

        assert_eq!(header1, header2);
        assert_ne!(header1, header3);
    }

    #[tokio::test]
    async fn test_mixed_transport_configurations() {
        let mut manager = McpServerManager::new();

        let configs = vec![
            McpServerConfig::Stdio(crate::config::StdioTransport {
                name: "stdio-server".to_string(),
                command: "/bin/echo".to_string(),
                args: vec!["stdio".to_string()],
                env: vec![crate::config::EnvVariable {
                    name: "TRANSPORT".to_string(),
                    value: "stdio".to_string(),
                }],
                cwd: None,
            }),
            // Note: HTTP and SSE will likely fail to connect in tests
            // but the manager should handle this gracefully
        ];

        let result = manager.connect_servers(configs).await;
        assert!(result.is_ok());

        // Should be able to shutdown cleanly regardless of connection failures
        let shutdown_result = manager.shutdown().await;
        assert!(shutdown_result.is_ok());
    }

    #[test]
    fn test_parse_sse_response_with_data() {
        let sse_body = "data: {\"jsonrpc\":\"2.0\",\"result\":{\"tools\":[]}}\n\n";
        
        let mut json_data = String::new();
        for line in sse_body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                json_data = data.to_string();
                break;
            }
        }
        
        assert!(!json_data.is_empty());
        let parsed: Value = serde_json::from_str(&json_data).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
    }

    #[test]
    fn test_parse_sse_response_empty() {
        let sse_body = "event: message\n\n";
        
        let mut json_data = String::new();
        for line in sse_body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                json_data = data.to_string();
                break;
            }
        }
        
        assert!(json_data.is_empty());
    }

    #[test]
    fn test_session_id_storage() {
        use tokio::runtime::Runtime;
        
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let session_id = Arc::new(RwLock::new(None));
            
            // Test initial state
            assert!(session_id.read().await.is_none());
            
            // Test storing session ID
            {
                let mut write_lock = session_id.write().await;
                *write_lock = Some("test-session-123".to_string());
            }
            
            // Test reading session ID
            let read_lock = session_id.read().await;
            assert_eq!(read_lock.as_ref().unwrap(), "test-session-123");
        });
    }

    #[test]
    fn test_http_transport_connection_has_session_id() {
        use tokio::runtime::Runtime;
        
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::new();
            let session_id = Arc::new(RwLock::new(Some("session-abc".to_string())));
            
            let transport = TransportConnection::Http {
                client: Arc::new(client),
                url: "http://localhost:8080".to_string(),
                headers: vec![],
                session_id: session_id.clone(),
            };
            
            match transport {
                TransportConnection::Http { session_id: sid, .. } => {
                    assert_eq!(sid.read().await.as_ref().unwrap(), "session-abc");
                }
                _ => panic!("Expected HTTP transport"),
            }
        });
    }
}
