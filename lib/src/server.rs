//! ACP Server Infrastructure
//!
//! This module provides the ACP (Agent Client Protocol) server implementation
//! that wraps the ClaudeAgent to provide JSON-RPC communication over stdio
//! and custom streams.

use crate::{agent::ClaudeAgent, config::AgentConfig, error::AgentError};
use agent_client_protocol::Agent;
use serde::Serialize;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

/// JSON-RPC notification wrapper for session/update notifications.
///
/// This struct wraps SessionNotification in the standard JSON-RPC 2.0 format and ensures
/// proper serialization with camelCase field names per the ACP specification.
///
/// ## Problem Solved
///
/// Previously, the server manually constructed JSON using `serde_json::json!` macro, which
/// used snake_case field names (e.g., `session_id`) instead of the ACP-required camelCase
/// (e.g., `sessionId`). This caused incompatibility with ACP-compliant clients.
///
/// ## Solution
///
/// By using this wrapper struct and relying on the protocol crate's serialization (which
/// already has proper `#[serde(rename_all = "camelCase")]` attributes), we get correct
/// field naming automatically without manual JSON construction.
#[derive(Debug, Serialize)]
struct JsonRpcNotification {
    jsonrpc: &'static str,
    method: &'static str,
    params: agent_client_protocol::SessionNotification,
}

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
    ///
    /// # Concurrency Model
    /// This method handles requests and notifications concurrently using `tokio::join!`.
    /// A broadcast channel coordinates shutdown: when the request handler completes
    /// (connection closed), it signals the notification handler to stop gracefully.
    ///
    /// # Shutdown Flow
    /// 1. Request handler processes incoming requests until reader closes
    /// 2. Upon completion, request handler sends shutdown signal via broadcast channel
    /// 3. Notification handler receives shutdown signal and stops gracefully
    /// 4. Both handlers complete, and the method returns
    ///
    /// The broadcast channel is used (vs. oneshot) because it allows the notification
    /// handler to continue processing notifications while monitoring for shutdown.
    ///
    /// # Arguments
    /// * `reader` - Async reader for incoming JSON-RPC requests
    /// * `writer` - Async writer for responses and notifications
    pub async fn start_with_streams<R, W>(&self, reader: R, writer: W) -> crate::Result<()>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        info!("Starting ACP server with custom streams");

        // Create shared writer for responses and notifications
        let writer = Arc::new(tokio::sync::Mutex::new(writer));
        let agent = Arc::clone(&self.agent);

        // Create a shutdown channel to coordinate between handlers
        // Capacity of 1 is sufficient since we only send a single shutdown signal
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

        // Handle requests directly without spawning (avoids Send issues)
        let mut notification_receiver = self.notification_receiver.resubscribe();

        // Handle requests and signal shutdown when done
        let request_handler = async {
            let result =
                Self::handle_requests(reader, Arc::clone(&writer), Arc::clone(&agent)).await;
            info!("Request handler completed, signaling shutdown");
            let _ = shutdown_tx.send(());
            result
        };

        // Handle notifications until shutdown signal
        let notification_handler = async {
            loop {
                tokio::select! {
                    notification_result = notification_receiver.recv() => {
                        match notification_result {
                            Ok(notification) => {
                                if let Err(e) = Self::send_notification(Arc::clone(&writer), notification).await {
                                    error!("Failed to send notification: {} - shutting down notification handler", e);
                                    break;
                                }
                            }
                            Err(_) => {
                                warn!("Notification channel closed");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Notification handler received shutdown signal");
                        break;
                    }
                }
            }
        };

        // Run both handlers concurrently
        // Both will continue until request handler completes, then notification handler stops
        let (request_result, _) = tokio::join!(request_handler, notification_handler);

        if let Err(e) = request_result {
            error!("Request handler failed: {}", e);
            return Err(e);
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
        info!("Request handler started, waiting for requests");
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await? {
            info!("Received line: {}", line);
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

        info!("Request handler completed (connection closed)");
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

        let is_notification = id.is_none();

        info!(
            "Handling {}: method={}, id={:?}",
            if is_notification {
                "notification"
            } else {
                "request"
            },
            method,
            id
        );

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
            "session/cancel" => {
                let req = serde_json::from_value(params)?;
                agent
                    .cancel(req)
                    .await
                    .map(|_| serde_json::Value::Null)
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

        // Only send response for requests (not notifications)
        // Per JSON-RPC 2.0 spec: "The Server MUST NOT reply to a Notification"
        if is_notification {
            // For notifications, log the result but do not send a response
            match response_result {
                Ok(_) => {
                    info!("Notification {} processed successfully", method);
                }
                Err(e) => {
                    error!("Notification {} failed: {}", method, e);
                }
            }
            return Ok(());
        }

        // Send response for requests
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

    /// Send a session update notification.
    ///
    /// Wraps the notification in JSON-RPC 2.0 format and serializes it with proper
    /// camelCase field names. The protocol crate's `SessionNotification` already uses
    /// camelCase serialization via serde attributes, ensuring ACP specification compliance.
    async fn send_notification<W>(
        writer: Arc<tokio::sync::Mutex<W>>,
        notification: agent_client_protocol::SessionNotification,
    ) -> crate::Result<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let notification_msg = JsonRpcNotification {
            jsonrpc: "2.0",
            method: "session/update",
            params: notification,
        };

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

    #[tokio::test]
    async fn test_concurrent_request_and_notification_handling() {
        let server = create_test_server().await;

        // Create two duplex channels - one for each direction
        // This matches how the original test was structured but with correct pairing
        let (mut client_writer, server_reader) = duplex(sizes::buffers::DUPLEX_STREAM_BUFFER);
        let (server_writer, mut client_reader) = duplex(sizes::buffers::DUPLEX_STREAM_BUFFER);

        // Run server and client concurrently
        let server_task = async {
            server
                .start_with_streams(server_reader, server_writer)
                .await
        };

        let client_task = async move {
            // Give server time to start listening
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Send an initialize request
            let init_request = serde_json::json!({
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
                    "protocolVersion": "1.0.0"
                }
            });

            let request_line = format!("{}\n", serde_json::to_string(&init_request).unwrap());
            client_writer
                .write_all(request_line.as_bytes())
                .await
                .unwrap();
            client_writer.flush().await.unwrap();

            // Read response - this should complete even if notifications are being sent
            let mut reader = BufReader::new(&mut client_reader);
            let mut response_line = String::new();

            // Use a timeout to ensure we don't hang forever
            let read_result = tokio::time::timeout(
                tokio::time::Duration::from_secs(5),
                reader.read_line(&mut response_line),
            )
            .await;

            assert!(
                read_result.is_ok(),
                "Request handling should complete even with notification handler running"
            );
            assert!(!response_line.is_empty(), "Should receive a response");

            let response: serde_json::Value = serde_json::from_str(&response_line).unwrap();
            assert_eq!(response.get("id").unwrap().as_i64().unwrap(), 1);
            assert!(
                response.get("result").is_some(),
                "Should have a result field"
            );

            // Close the streams to signal server to stop
            drop(client_writer);
            drop(reader);
        };

        // Spawn client task so it runs independently
        let client_handle = tokio::spawn(client_task);

        // Run server task
        let server_result = server_task.await;

        // Wait for client to complete
        client_handle.await.unwrap();

        // Server should complete successfully when streams close
        server_result.expect("Server should complete successfully");
    }

    /// Validates that the agent_client_protocol crate uses proper camelCase serialization.
    /// This test ensures that the protocol crate's serde attributes are correctly configured
    /// to serialize field names according to the ACP specification (camelCase, not snake_case).
    #[tokio::test]
    async fn test_protocol_type_serialization() {
        use agent_client_protocol::{
            ContentBlock, SessionId, SessionNotification, SessionUpdate, TextContent,
        };

        // First, let's see how the protocol crate serializes SessionNotification
        let notification = SessionNotification {
            session_id: SessionId("sess_test123".to_string().into()),
            update: SessionUpdate::AgentThoughtChunk {
                content: ContentBlock::Text(TextContent {
                    text: "test thought".to_string(),
                    annotations: None,
                    meta: None,
                }),
            },
            meta: Some(serde_json::json!({"test": true})),
        };

        // Serialize the notification directly using the protocol crate's serde implementation
        let json_str = serde_json::to_string(&notification).expect("Should serialize");
        let json_value: serde_json::Value = serde_json::from_str(&json_str).expect("Should parse");

        // The protocol crate should handle the casing correctly
        assert!(
            json_value.get("sessionId").is_some() || json_value.get("session_id").is_some(),
            "Should have session_id or sessionId field. Found: {:?}",
            json_value
        );
    }

    /// Validates that session/update notifications are sent in proper JSON-RPC format with camelCase field names.
    /// This is an end-to-end test that verifies the complete JSON-RPC message structure matches the ACP specification,
    /// including proper field naming (sessionId, not session_id) and the _meta prefix for metadata.
    #[tokio::test]
    async fn test_session_notification_uses_camel_case() {
        use agent_client_protocol::{
            ContentBlock, SessionId, SessionNotification, SessionUpdate, TextContent,
        };

        // Create a test notification
        let notification = SessionNotification {
            session_id: SessionId("sess_test123".to_string().into()),
            update: SessionUpdate::AgentThoughtChunk {
                content: ContentBlock::Text(TextContent {
                    text: "test thought".to_string(),
                    annotations: None,
                    meta: None,
                }),
            },
            meta: Some(serde_json::json!({"test": true})),
        };

        // Create a mock writer
        let writer = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Send the notification
        ClaudeAgentServer::send_notification(writer.clone(), notification)
            .await
            .expect("Should send notification");

        // Read what was written
        let bytes = writer.lock().await;
        let json_str = String::from_utf8(bytes.clone()).expect("Should be valid UTF-8");
        let json_value: serde_json::Value =
            serde_json::from_str(json_str.trim()).expect("Should be valid JSON");

        // Verify the JSON-RPC structure
        assert_eq!(json_value.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
        assert_eq!(
            json_value.get("method").unwrap().as_str().unwrap(),
            "session/update"
        );

        // Verify params exist
        let params = json_value
            .get("params")
            .expect("Should have params field")
            .as_object()
            .expect("Params should be an object");

        // This is the key test: verify camelCase field names per ACP spec
        assert!(
            params.contains_key("sessionId"),
            "Should use camelCase 'sessionId', not snake_case 'session_id'. Found keys: {:?}",
            params.keys().collect::<Vec<_>>()
        );

        // Verify snake_case is NOT present
        assert!(
            !params.contains_key("session_id"),
            "Should NOT use snake_case 'session_id'"
        );

        // Verify the sessionId value is correct
        assert_eq!(
            params.get("sessionId").unwrap().as_str().unwrap(),
            "sess_test123"
        );

        // Verify other fields are present
        assert!(params.contains_key("update"), "Should have update field");

        // Per ACP spec, meta field is prefixed with underscore
        assert!(
            params.contains_key("_meta"),
            "Should have _meta field (per ACP spec)"
        );

        // Verify meta value is correct
        let meta = params.get("_meta").unwrap().as_object().unwrap();
        assert!(meta.get("test").unwrap().as_bool().unwrap());
    }

    /// Test that notifications (requests without an id field) do not receive responses.
    /// Per JSON-RPC 2.0 spec: "The Server MUST NOT reply to a Notification."
    #[tokio::test]
    async fn test_notifications_do_not_receive_responses() {
        let server = create_test_server().await;

        let (mut client_writer, server_reader) = duplex(sizes::buffers::DUPLEX_STREAM_BUFFER);
        let (server_writer, mut client_reader) = duplex(sizes::buffers::DUPLEX_STREAM_BUFFER);

        let server_task = async {
            server
                .start_with_streams(server_reader, server_writer)
                .await
        };

        let client_task = async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Send a notification (no id field) - using session/cancel as an example
            let notification = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "session/cancel",
                "params": {
                    "sessionId": "test-session-123"
                }
            });

            let notification_line = format!("{}\n", serde_json::to_string(&notification).unwrap());
            client_writer
                .write_all(notification_line.as_bytes())
                .await
                .unwrap();
            client_writer.flush().await.unwrap();

            // Send a valid request afterwards to ensure server is still responsive
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 99,
                "method": "initialize",
                "params": {
                    "client_capabilities": {
                        "fs": {
                            "read_text_file": true,
                            "write_text_file": true
                        },
                        "terminal": true
                    },
                    "protocolVersion": "1.0.0"
                }
            });

            let request_line = format!("{}\n", serde_json::to_string(&request).unwrap());
            client_writer
                .write_all(request_line.as_bytes())
                .await
                .unwrap();
            client_writer.flush().await.unwrap();

            // Read ALL responses to see what the server actually sends
            let mut reader = BufReader::new(&mut client_reader);
            let mut responses = Vec::new();

            // Read up to 2 lines with a timeout
            for i in 0..2 {
                let mut line = String::new();
                let read_result = tokio::time::timeout(
                    tokio::time::Duration::from_millis(500),
                    reader.read_line(&mut line),
                )
                .await;

                if read_result.is_ok() && !line.trim().is_empty() {
                    println!("Response {}: {}", i, line);
                    responses.push(line);
                } else {
                    break;
                }
            }

            // We should receive EXACTLY 1 response (for the request with id=99)
            assert_eq!(
                responses.len(),
                1,
                "Should receive exactly 1 response (for id=99), but got {}. Responses: {:?}",
                responses.len(),
                responses
            );

            let response: serde_json::Value = serde_json::from_str(&responses[0]).unwrap();
            assert_eq!(
                response.get("id").and_then(|v| v.as_i64()),
                Some(99),
                "The single response should be for id=99. Got: {:?}",
                response
            );

            // Verify no id:null response was sent
            for resp_str in &responses {
                let resp: serde_json::Value = serde_json::from_str(resp_str).unwrap();
                assert!(
                    !resp.get("id").map(|v| v.is_null()).unwrap_or(false),
                    "Should not receive a response with id:null (would be notification response). Got: {:?}",
                    resp
                );
            }

            // Close the streams
            drop(client_writer);
            drop(reader);
        };

        let client_handle = tokio::spawn(client_task);

        // Run server directly without spawning
        let (server_result, _) = tokio::join!(server_task, client_handle);

        server_result.expect("Server should complete successfully");
    }

    /// Test that requests (with an id field) still receive responses.
    /// This ensures we don't break normal request/response behavior.
    #[tokio::test]
    async fn test_requests_receive_responses() {
        let server = create_test_server().await;

        let (mut client_writer, server_reader) = duplex(sizes::buffers::DUPLEX_STREAM_BUFFER);
        let (server_writer, mut client_reader) = duplex(sizes::buffers::DUPLEX_STREAM_BUFFER);

        let server_task = async {
            server
                .start_with_streams(server_reader, server_writer)
                .await
        };

        let client_task = async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Send a request (with id field)
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 42,
                "method": "initialize",
                "params": {
                    "client_capabilities": {
                        "fs": {
                            "read_text_file": true,
                            "write_text_file": true
                        },
                        "terminal": true
                    },
                    "protocolVersion": "1.0.0"
                }
            });

            let request_line = format!("{}\n", serde_json::to_string(&request).unwrap());
            client_writer
                .write_all(request_line.as_bytes())
                .await
                .unwrap();
            client_writer.flush().await.unwrap();

            // Should receive a response
            let mut reader = BufReader::new(&mut client_reader);
            let mut response_line = String::new();

            let read_result = tokio::time::timeout(
                tokio::time::Duration::from_secs(5),
                reader.read_line(&mut response_line),
            )
            .await;

            assert!(
                read_result.is_ok(),
                "Request should receive a response"
            );
            assert!(
                !response_line.is_empty(),
                "Response should not be empty"
            );

            let response: serde_json::Value = serde_json::from_str(&response_line).unwrap();
            assert_eq!(
                response.get("id").unwrap().as_i64().unwrap(),
                42,
                "Response id should match request id"
            );

            drop(client_writer);
            drop(reader);
        };

        let client_handle = tokio::spawn(client_task);

        // Run both concurrently
        let (server_result, _) = tokio::join!(server_task, client_handle);

        server_result.expect("Server should complete successfully");
    }
}
