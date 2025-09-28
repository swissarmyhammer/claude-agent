//! Enhanced MCP server connection error handling for ACP compliance
//! 
//! This module provides comprehensive error handling for MCP server connections
//! following ACP specification requirements with detailed error reporting.

use crate::{
    config::McpServerConfig, 
    mcp::{McpServerConnection, TransportConnection},
    session_errors::{SessionSetupError, SessionSetupResult},
    session_validation::validate_mcp_server_config,
};
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

/// Enhanced MCP server connection manager with comprehensive error handling
pub struct EnhancedMcpServerManager {
    /// Map of server name to connection
    connections: Arc<RwLock<HashMap<String, McpServerConnection>>>,
    /// Connection timeout in milliseconds
    connection_timeout_ms: u64,
    /// Protocol negotiation timeout in milliseconds  
    protocol_timeout_ms: u64,
}

impl Default for EnhancedMcpServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnhancedMcpServerManager {
    /// Create a new enhanced MCP server manager with default timeouts
    pub fn new() -> Self {
        Self::with_timeouts(30000, 10000) // 30s connection, 10s protocol
    }

    /// Create a new enhanced MCP server manager with custom timeouts
    pub fn with_timeouts(connection_timeout_ms: u64, protocol_timeout_ms: u64) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_timeout_ms,
            protocol_timeout_ms,
        }
    }

    /// Connect to all configured MCP servers with comprehensive error handling
    ///
    /// This method validates each server configuration before attempting connection
    /// and provides detailed error information for each failure while continuing
    /// to connect to other servers.
    pub async fn connect_servers_with_validation(
        &mut self, 
        configs: Vec<McpServerConfig>
    ) -> SessionSetupResult<HashMap<String, Result<String, SessionSetupError>>> {
        let mut results = HashMap::new();
        
        for config in configs {
            let server_name = config.name().to_string();
            
            // Step 1: Validate server configuration before attempting connection
            match validate_mcp_server_config(&config) {
                Ok(()) => {
                    // Step 2: Attempt connection with comprehensive error handling
                    match self.connect_server_enhanced(config).await {
                        Ok(connection) => {
                            tracing::info!(
                                "Successfully connected to MCP server: {} with {} tools", 
                                connection.name, 
                                connection.tools.len()
                            );
                            let mut connections = self.connections.write().await;
                            connections.insert(connection.name.clone(), connection);
                            results.insert(server_name, Ok("Connected successfully".to_string()));
                        }
                        Err(e) => {
                            tracing::error!("Failed to connect to MCP server {}: {}", server_name, e);
                            results.insert(server_name, Err(e));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Invalid MCP server configuration for {}: {}", server_name, e);
                    results.insert(server_name, Err(e));
                }
            }
        }
        
        Ok(results)
    }

    /// Connect to a single MCP server with enhanced error handling
    async fn connect_server_enhanced(&self, config: McpServerConfig) -> SessionSetupResult<McpServerConnection> {
        let start_time = Instant::now();
        
        match config.clone() {
            McpServerConfig::Stdio(stdio_config) => {
                self.connect_stdio_server_enhanced(config, &stdio_config, start_time).await
            }
            McpServerConfig::Http(http_config) => {
                self.connect_http_server_enhanced(config, &http_config, start_time).await
            }
            McpServerConfig::Sse(sse_config) => {
                self.connect_sse_server_enhanced(config, &sse_config, start_time).await
            }
        }
    }

    /// Connect to STDIO MCP server with comprehensive error handling
    async fn connect_stdio_server_enhanced(
        &self,
        config: McpServerConfig,
        stdio_config: &crate::config::StdioTransport,
        start_time: Instant,
    ) -> SessionSetupResult<McpServerConnection> {
        tracing::info!(
            "Attempting STDIO connection to MCP server: {} ({})",
            stdio_config.name,
            stdio_config.command
        );

        // Build the command with comprehensive error handling
        let mut command = Command::new(&stdio_config.command);
        command.args(&stdio_config.args);

        // Set working directory if provided with validation
        if let Some(cwd_str) = &stdio_config.cwd {
            let cwd_path = std::path::Path::new(cwd_str);
            if !cwd_path.exists() {
                return Err(SessionSetupError::McpServerConnectionFailed {
                    server_name: stdio_config.name.clone(),
                    error: format!("Working directory does not exist: {}", cwd_path.display()),
                    transport_type: "stdio".to_string(),
                });
            }
            command.current_dir(cwd_path);
        }

        // Set environment variables
        for env_var in &stdio_config.env {
            command.env(&env_var.name, &env_var.value);
        }

        // Configure process stdio
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())  
            .stderr(Stdio::piped());

        // Spawn the process with detailed error handling
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                return match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        Err(SessionSetupError::McpServerExecutableNotFound {
                            server_name: stdio_config.name.clone(),
                            command: Path::new(&stdio_config.command).to_path_buf(),
                            suggestion: if Path::new(&stdio_config.command).is_absolute() {
                                "Check that the executable exists and has proper permissions".to_string()
                            } else {
                                format!("Install {} or provide the full path to the executable", stdio_config.command)
                            },
                        })
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        Err(SessionSetupError::McpServerConnectionFailed {
                            server_name: stdio_config.name.clone(),
                            error: "Permission denied: insufficient permissions to execute server".to_string(),
                            transport_type: "stdio".to_string(),
                        })
                    }
                    _ => {
                        Err(SessionSetupError::McpServerStartupFailed {
                            server_name: stdio_config.name.clone(),
                            exit_code: -1,
                            stderr: format!("Process spawn failed: {}", e),
                            suggestion: "Check server installation, permissions, and system resources".to_string(),
                        })
                    }
                }
            }
        };

        // Check if process started successfully (hasn't exited immediately)
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(Some(exit_status)) = child.try_wait() {
            // Process exited immediately - likely a startup failure
            let stderr_output = if let Some(stderr) = child.stderr.take() {
                let mut stderr_reader = BufReader::new(stderr);
                let mut stderr_content = Vec::new();
                let _ = stderr_reader.read_to_end(&mut stderr_content).await;
                String::from_utf8_lossy(&stderr_content).to_string()
            } else {
                "No stderr available".to_string()
            };

            return Err(SessionSetupError::McpServerStartupFailed {
                server_name: stdio_config.name.clone(),
                exit_code: exit_status.code().unwrap_or(-1),
                stderr: stderr_output,
                suggestion: "Check server logs and configuration".to_string(),
            });
        }

        // Get stdio handles
        let stdin = child.stdin.take().ok_or_else(|| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: stdio_config.name.clone(),
                error: "Failed to get stdin handle from child process".to_string(),
                transport_type: "stdio".to_string(),
            }
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: stdio_config.name.clone(),
                error: "Failed to get stdout handle from child process".to_string(),
                transport_type: "stdio".to_string(),
            }
        })?;

        let mut stdin_writer = BufWriter::new(stdin);
        let mut stdout_reader = BufReader::new(stdout);

        // Initialize MCP protocol with timeout and comprehensive error handling
        let tools = match timeout(
            Duration::from_millis(self.protocol_timeout_ms),
            self.initialize_mcp_protocol_enhanced(
                &mut stdin_writer,
                &mut stdout_reader,
                &stdio_config.name,
            ),
        ).await {
            Ok(Ok(tools)) => tools,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                // Kill the child process on timeout
                let _ = child.kill().await;
                return Err(SessionSetupError::McpServerTimeout {
                    server_name: stdio_config.name.clone(),
                    timeout_ms: self.protocol_timeout_ms,
                    transport_type: "stdio_protocol".to_string(),
                });
            }
        };

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

        let connection_time = start_time.elapsed();
        tracing::info!(
            "Successfully connected to STDIO MCP server {} in {:?}",
            stdio_config.name,
            connection_time
        );

        Ok(connection)
    }

    /// Connect to HTTP MCP server with comprehensive error handling
    async fn connect_http_server_enhanced(
        &self,
        config: McpServerConfig,
        http_config: &crate::config::HttpTransport,
        _start_time: Instant,
    ) -> SessionSetupResult<McpServerConnection> {
        tracing::info!(
            "Attempting HTTP connection to MCP server: {} ({})",
            http_config.name,
            http_config.url
        );

        // Validate and parse URL
        let parsed_url = Url::parse(&http_config.url).map_err(|_| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: "Invalid URL format".to_string(),
                transport_type: "http".to_string(),
            }
        })?;

        // Build HTTP client with headers
        let mut headers = reqwest::header::HeaderMap::new();
        for header in &http_config.headers {
            let name = reqwest::header::HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|_| {
                    SessionSetupError::McpServerConnectionFailed {
                        server_name: http_config.name.clone(),
                        error: format!("Invalid header name: {}", header.name),
                        transport_type: "http".to_string(),
                    }
                })?;
            
            let value = reqwest::header::HeaderValue::from_str(&header.value)
                .map_err(|_| {
                    SessionSetupError::McpServerConnectionFailed {
                        server_name: http_config.name.clone(),
                        error: format!("Invalid header value: {}", header.value),
                        transport_type: "http".to_string(),
                    }
                })?;
            
            headers.insert(name, value);
        }

        let client = Client::builder()
            .timeout(Duration::from_millis(self.connection_timeout_ms))
            .default_headers(headers)
            .build()
            .map_err(|e| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("Failed to create HTTP client: {}", e),
                    transport_type: "http".to_string(),
                }
            })?;

        // Test connection and initialize protocol
        let tools = self.initialize_http_mcp_protocol_enhanced(&client, http_config).await?;

        let transport = TransportConnection::Http {
            client: Arc::new(client),
            url: parsed_url.to_string(),
            headers: http_config.headers.clone(),
        };

        let connection = McpServerConnection {
            name: http_config.name.clone(),
            tools,
            config,
            transport,
        };

        Ok(connection)
    }

    /// Connect to SSE MCP server with comprehensive error handling
    async fn connect_sse_server_enhanced(
        &self,
        config: McpServerConfig,
        sse_config: &crate::config::SseTransport,
        _start_time: Instant,
    ) -> SessionSetupResult<McpServerConnection> {
        tracing::info!(
            "Attempting SSE connection to MCP server: {} ({})",
            sse_config.name,
            sse_config.url
        );

        // Validate URL format
        Url::parse(&sse_config.url).map_err(|_| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: sse_config.name.clone(),
                error: "Invalid URL format".to_string(),
                transport_type: "sse".to_string(),
            }
        })?;

        // Create SSE connection channels
        let (message_tx, _message_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        // Initialize SSE connection
        let tools = self.initialize_sse_mcp_protocol_enhanced(sse_config, response_tx).await?;

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

    /// Initialize MCP protocol with comprehensive error handling
    async fn initialize_mcp_protocol_enhanced(
        &self,
        writer: &mut BufWriter<tokio::process::ChildStdin>,
        reader: &mut BufReader<tokio::process::ChildStdout>,
        server_name: &str,
    ) -> SessionSetupResult<Vec<String>> {
        // Send initialize request
        let initialize_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {
                        "listChanged": true
                    },
                    "sampling": {}
                },
                "clientInfo": {
                    "name": "claude-agent",
                    "version": "1.0.0"
                }
            }
        });

        let request_line = format!("{}\n", initialize_request);
        
        writer.write_all(request_line.as_bytes()).await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to send initialize request: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;
        
        writer.flush().await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to flush initialize request: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;

        // Read initialize response
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.map_err(|_e| {
            SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: None,
            }
        })?;

        if response_line.trim().is_empty() {
            return Err(SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: Some("No response".to_string()),
            });
        }

        let response: Value = serde_json::from_str(response_line.trim()).map_err(|e| {
            SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: Some(format!("Invalid JSON: {}", e)),
            }
        })?;

        // Validate initialize response
        if let Some(error) = response.get("error") {
            return Err(SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: Some(format!("Server error: {}", error)),
            });
        }

        // Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let notification_line = format!("{}\n", initialized_notification);
        writer.write_all(notification_line.as_bytes()).await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to send initialized notification: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;
        
        writer.flush().await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to flush initialized notification: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;

        // Request list of tools
        let tools_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let tools_line = format!("{}\n", tools_request);
        writer.write_all(tools_line.as_bytes()).await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to send tools/list request: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;
        
        writer.flush().await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to flush tools/list request: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;

        // Read tools response
        let mut tools_response_line = String::new();
        reader.read_line(&mut tools_response_line).await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to read tools/list response: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;

        let tools_response: Value = serde_json::from_str(tools_response_line.trim()).map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Invalid tools/list response JSON: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;

        // Extract tool names from response
        let tools = if let Some(result) = tools_response.get("result") {
            if let Some(tools_array) = result.get("tools").and_then(|t| t.as_array()) {
                tools_array
                    .iter()
                    .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()))
                    .map(|name| name.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        tracing::info!("MCP server {} reported {} tools", server_name, tools.len());
        Ok(tools)
    }

    /// Initialize HTTP MCP protocol with comprehensive error handling
    async fn initialize_http_mcp_protocol_enhanced(
        &self,
        client: &Client,
        http_config: &crate::config::HttpTransport,
    ) -> SessionSetupResult<Vec<String>> {
        // This is a placeholder implementation - HTTP MCP protocol would need actual specification
        tracing::warn!("HTTP MCP protocol initialization is not fully implemented");
        
        // Test basic connectivity
        let test_response = client
            .get(&http_config.url)
            .send()
            .await
            .map_err(|e| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("HTTP connection failed: {}", e),
                    transport_type: "http".to_string(),
                }
            })?;

        if !test_response.status().is_success() {
            return Err(SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: format!("HTTP connection failed with status: {}", test_response.status()),
                transport_type: "http".to_string(),
            });
        }

        Ok(vec!["http_placeholder_tool".to_string()])
    }

    /// Initialize SSE MCP protocol with comprehensive error handling
    async fn initialize_sse_mcp_protocol_enhanced(
        &self,
        _sse_config: &crate::config::SseTransport,
        _response_tx: mpsc::UnboundedSender<String>,
    ) -> SessionSetupResult<Vec<String>> {
        // This is a placeholder implementation - SSE MCP protocol would need actual specification
        tracing::warn!("SSE MCP protocol initialization is not fully implemented");
        
        // Basic URL validation was already done in connect_sse_server_enhanced
        Ok(vec!["sse_placeholder_tool".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_manager_creation() {
        let manager = EnhancedMcpServerManager::new();
        let connections = manager.connections.read().await;
        assert!(connections.is_empty());
    }

    #[tokio::test]
    async fn test_connect_servers_with_invalid_config() {
        let mut manager = EnhancedMcpServerManager::new();
        
        let invalid_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "invalid_server".to_string(),
            command: "/nonexistent/command".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });
        
        let results = manager.connect_servers_with_validation(vec![invalid_config]).await.unwrap();
        
        assert_eq!(results.len(), 1);
        assert!(results.get("invalid_server").unwrap().is_err());
    }

    #[tokio::test]
    async fn test_stdio_server_nonexistent_command() {
        let manager = EnhancedMcpServerManager::new();
        
        let config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "test_server".to_string(),
            command: "/absolutely/nonexistent/command".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });
        
        let result = manager.connect_server_enhanced(config).await;
        assert!(result.is_err());
        
        if let Err(SessionSetupError::McpServerExecutableNotFound { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected McpServerExecutableNotFound error");
        }
    }

    #[tokio::test]
    async fn test_http_server_invalid_url() {
        let manager = EnhancedMcpServerManager::new();
        
        let config = McpServerConfig::Http(crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test_server".to_string(),
            url: "not-a-valid-url".to_string(),
            headers: vec![],
        });
        
        let result = manager.connect_server_enhanced(config).await;
        assert!(result.is_err());
        
        if let Err(SessionSetupError::McpServerConnectionFailed { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected McpServerConnectionFailed error");
        }
    }
}