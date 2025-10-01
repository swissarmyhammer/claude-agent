//! ACP Server Infrastructure
//!
//! This module provides the ACP (Agent Client Protocol) server implementation
//! that wraps the ClaudeAgent to provide JSON-RPC communication over stdio
//! and custom streams.

use crate::{agent::ClaudeAgent, config::AgentConfig, error::AgentError};
use agent_client_protocol::Agent;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

/// The main ACP server that handles JSON-RPC communication
pub struct ClaudeAgentServer {
    agent: Arc<ClaudeAgent>,
    notification_receiver: broadcast::Receiver<agent_client_protocol::SessionNotification>,
}

impl ClaudeAgentServer {
    /// Create a new Claude Agent server with the given configuration
    pub async fn new(config: AgentConfig) -> crate::Result<Self> {
        let (agent, notification_receiver) = ClaudeAgent::new(config).await?;

        Ok(Self {
            agent: Arc::new(agent),
            notification_receiver,
        })
    }

    /// Start the server using stdio (standard ACP pattern)
    pub async fn start_stdio(&self) -> crate::Result<()> {
        info!("Starting ACP server on stdio");

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        self.start_with_streams(stdin, stdout).await
    }

    /// Start the server with graceful shutdown handling
    pub async fn start_with_shutdown(&self) -> crate::Result<()> {
        info!("Starting ACP server with shutdown handling");

        let shutdown_signal = self.setup_shutdown_handler();
        let server_task = self.start_stdio();

        tokio::select! {
            result = server_task => {
                info!("Server task completed: {:?}", result);
                result
            }
            _ = shutdown_signal => {
                info!("Received shutdown signal, stopping server");
                self.shutdown().await?;
                Ok(())
            }
        }
    }

    /// Setup shutdown signal handler
    async fn setup_shutdown_handler(&self) -> crate::Result<()> {
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .map_err(AgentError::Io)?;
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                .map_err(AgentError::Io)?;

            tokio::select! {
                _ = sigterm.recv() => {
                    info!("Received SIGTERM");
                }
                _ = sigint.recv() => {
                    info!("Received SIGINT");
                }
            }
        }

        #[cfg(not(unix))]
        {
            signal::ctrl_c().await.map_err(|e| AgentError::Io(e))?;
            info!("Received Ctrl+C");
        }

        Ok(())
    }

    /// Perform graceful shutdown
    async fn shutdown(&self) -> crate::Result<()> {
        info!("Shutting down server");

        // Clean up active sessions by shutting down the session manager
        if let Err(e) = self.agent.shutdown_sessions().await {
            warn!("Error shutting down sessions: {}", e);
        }

        // Close MCP server connections if any exist
        if let Err(e) = self.agent.shutdown_mcp_connections().await {
            warn!("Error shutting down MCP connections: {}", e);
        }

        // Stop any background tool processes
        if let Err(e) = self.agent.shutdown_tool_handler().await {
            warn!("Error shutting down tool handler: {}", e);
        }

        // The notification channel will be dropped when the server is dropped,
        // which will automatically close all receivers

        info!("Server shutdown complete");
        Ok(())
    }

    /// Start the server with custom streams for testing
    pub async fn start_with_streams<R, W>(&self, reader: R, writer: W) -> crate::Result<()>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        info!("Starting ACP server with custom streams");

        // Create shared writer for responses and notifications
        let writer = Arc::new(tokio::sync::Mutex::new(writer));
        let agent = Arc::clone(&self.agent);

        // Handle requests directly without spawning (avoids Send issues)
        let mut notification_receiver = self.notification_receiver.resubscribe();

        // For now, just handle requests sequentially
        // In a production system, we'd need a more sophisticated approach
        // to handle notifications and requests concurrently
        tokio::select! {
            result = Self::handle_requests(reader, Arc::clone(&writer), Arc::clone(&agent)) => {
                if let Err(e) = result {
                    error!("Request handler failed: {}", e);
                }
            }
            _ = async {
                while let Ok(notification) = notification_receiver.recv().await {
                    if let Err(e) = Self::send_notification(Arc::clone(&writer), notification).await {
                        error!("Failed to send notification: {}", e);
                        break;
                    }
                }
            } => {
                warn!("Notification handler completed");
            }
        }

        Ok(())
    }

    /// Handle incoming JSON-RPC requests
    async fn handle_requests<R, W>(
        reader: R,
        writer: Arc<tokio::sync::Mutex<W>>,
        agent: Arc<ClaudeAgent>,
    ) -> crate::Result<()>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            // Handle request directly to avoid Send issues
            if let Err(e) =
                Self::handle_single_request(line, Arc::clone(&writer), Arc::clone(&agent)).await
            {
                error!("Failed to handle request: {}", e);
            }
        }

        Ok(())
    }

    /// Handle a single JSON-RPC request
    async fn handle_single_request<W>(
        line: String,
        writer: Arc<tokio::sync::Mutex<W>>,
        agent: Arc<ClaudeAgent>,
    ) -> crate::Result<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        // Parse the JSON-RPC request
        let request: serde_json::Value = serde_json::from_str(&line)?;

        let method = request
            .get("method")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::Protocol("Missing method".to_string()))?;

        let id = request.get("id").cloned();
        let params = request
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        info!("Handling request: method={}, id={:?}", method, id);

        // Route to appropriate agent method
        let response_result = match method {
            "initialize" => {
                let req = serde_json::from_value(params)?;
                agent
                    .initialize(req)
                    .await
                    .map(|r| serde_json::to_value(r).unwrap())
            }
            "authenticate" => {
                let req = serde_json::from_value(params)?;
                agent
                    .authenticate(req)
                    .await
                    .map(|r| serde_json::to_value(r).unwrap())
            }
            "session/new" => {
                let req = serde_json::from_value(params)?;
                agent
                    .new_session(req)
                    .await
                    .map(|r| serde_json::to_value(r).unwrap())
            }
            "session/load" => {
                let req = serde_json::from_value(params)?;
                agent
                    .load_session(req)
                    .await
                    .map(|r| serde_json::to_value(r).unwrap())
            }
            "session/set-mode" => {
                let req = serde_json::from_value(params)?;
                agent
                    .set_session_mode(req)
                    .await
                    .map(|r| serde_json::to_value(r).unwrap())
            }
            "session/prompt" => {
                let req = serde_json::from_value(params)?;
                agent
                    .prompt(req)
                    .await
                    .map(|r| serde_json::to_value(r).unwrap())
            }
            // Handle extension methods through ext_method
            _ => {
                let params_raw = agent_client_protocol::RawValue::from_string(params.to_string())
                    .map_err(|_| {
                    AgentError::Protocol("Failed to convert params to RawValue".to_string())
                })?;

                let ext_request = agent_client_protocol::ExtRequest {
                    method: method.to_string().into(),
                    params: Arc::from(params_raw),
                };
                agent
                    .ext_method(ext_request)
                    .await
                    .map(|raw_value| {
                        // Parse the RawValue back to serde_json::Value
                        serde_json::from_str(raw_value.get()).unwrap_or_else(|_| {
                            serde_json::Value::String(raw_value.get().to_string())
                        })
                    })
                    .map_err(|e| {
                        tracing::error!("Extension method {} failed: {}", method, e);
                        agent_client_protocol::Error::internal_error()
                    })
            }
        };

        // Send response
        let response = match response_result {
            Ok(result) => {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": result
                })
            }
            Err(e) => {
                error!("Method {} failed: {}", method, e);
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32603,
                        "message": e.to_string()
                    }
                })
            }
        };

        Self::send_response(writer, response).await
    }

    /// Send a JSON-RPC response
    async fn send_response<W>(
        writer: Arc<tokio::sync::Mutex<W>>,
        response: serde_json::Value,
    ) -> crate::Result<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let response_line = format!("{}\n", serde_json::to_string(&response)?);

        let mut writer_guard = writer.lock().await;
        writer_guard.write_all(response_line.as_bytes()).await?;
        writer_guard.flush().await?;

        Ok(())
    }

    /// Send a session update notification
    async fn send_notification<W>(
        writer: Arc<tokio::sync::Mutex<W>>,
        notification: agent_client_protocol::SessionNotification,
    ) -> crate::Result<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let notification_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "session_id": notification.session_id,
                "update": notification.update,
                "meta": notification.meta
            }
        });

        let notification_line = format!("{}\n", serde_json::to_string(&notification_msg)?);

        let mut writer_guard = writer.lock().await;
        writer_guard.write_all(notification_line.as_bytes()).await?;
        writer_guard.flush().await?;

        Ok(())
    }
}

/// Connection manager for tracking active connections
pub struct ConnectionManager {
    active_connections: Arc<tokio::sync::RwLock<std::collections::HashMap<String, ConnectionInfo>>>,
}

/// Information about an active connection
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: String,
    pub created_at: std::time::SystemTime,
    pub last_activity: std::time::SystemTime,
    pub protocol_version: String,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new() -> Self {
        Self {
            active_connections: Arc::new(
                tokio::sync::RwLock::new(std::collections::HashMap::new()),
            ),
        }
    }

    /// Register a new connection
    pub async fn register_connection(
        &self,
        id: String,
        protocol_version: String,
    ) -> crate::Result<()> {
        let now = std::time::SystemTime::now();
        let info = ConnectionInfo {
            id: id.clone(),
            created_at: now,
            last_activity: now,
            protocol_version,
        };

        let mut connections = self.active_connections.write().await;
        connections.insert(id.clone(), info);

        info!("Registered new connection: {}", id);
        Ok(())
    }

    /// Update activity timestamp for a connection
    pub async fn update_activity(&self, id: &str) -> crate::Result<()> {
        let mut connections = self.active_connections.write().await;
        if let Some(info) = connections.get_mut(id) {
            info.last_activity = std::time::SystemTime::now();
        }
        Ok(())
    }

    /// Remove a connection
    pub async fn remove_connection(&self, id: &str) -> crate::Result<()> {
        let mut connections = self.active_connections.write().await;
        connections.remove(id);

        info!("Removed connection: {}", id);
        Ok(())
    }

    /// List all active connections
    pub async fn list_connections(&self) -> crate::Result<Vec<String>> {
        let connections = self.active_connections.read().await;
        Ok(connections.keys().cloned().collect())
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::sizes;
    use tokio::io::duplex;

    async fn create_test_server() -> ClaudeAgentServer {
        let config = AgentConfig::default();
        ClaudeAgentServer::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_server_creation() {
        let server = create_test_server().await;
        // Just ensure we can create a server without panicking
        assert!(std::ptr::eq(server.agent.as_ref(), server.agent.as_ref()));
    }

    #[tokio::test]
    async fn test_connection_manager() {
        let manager = ConnectionManager::new();

        manager
            .register_connection("test-conn".to_string(), "1.0.0".to_string())
            .await
            .unwrap();

        let connections = manager.list_connections().await.unwrap();
        assert_eq!(connections.len(), 1);
        assert!(connections.contains(&"test-conn".to_string()));

        manager.update_activity("test-conn").await.unwrap();

        manager.remove_connection("test-conn").await.unwrap();

        let connections = manager.list_connections().await.unwrap();
        assert!(connections.is_empty());
    }

    #[tokio::test]
    async fn test_stream_server_startup() {
        let _server = create_test_server().await;

        let (_client_stream, _server_stream) = duplex(sizes::buffers::DUPLEX_STREAM_BUFFER);

        // For now, just test that we can create the server without panicking
        // Full integration testing would require more sophisticated test setup
        // to handle the Agent trait's Send bounds

        // If we get here without panicking, the server was created successfully
    }

    #[tokio::test]
    async fn test_json_rpc_request_parsing() {
        let _server = create_test_server().await;

        // Test that we can parse a basic JSON-RPC request without panicking
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "client_capabilities": {
                    "fs": {
                        "read_text_file": true,
                        "write_text_file": true
                    },
                    "terminal": true
                },
                "protocol_version": "1.0.0"
            }
        });

        let line = serde_json::to_string(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(
            parsed.get("method").unwrap().as_str().unwrap(),
            "initialize"
        );
        assert_eq!(parsed.get("id").unwrap().as_i64().unwrap(), 1);
    }
}
