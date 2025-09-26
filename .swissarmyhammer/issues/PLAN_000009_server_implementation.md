# ACP Server Infrastructure

Refer to plan.md

## Goal
Build the ACP server infrastructure with generic streams, stdio transport, and connection management.

## Tasks

### 1. Server Structure (`lib/src/server.rs`)

```rust
use agent_client_protocol::{Agent, JsonRpcTransport};
use tokio::io::{AsyncRead, AsyncWrite};
use std::sync::Arc;
use crate::agent::ClaudeAgent;

pub struct ClaudeAgentServer {
    agent: Arc<ClaudeAgent>,
    notification_receiver: tokio::sync::broadcast::Receiver<agent_client_protocol::SessionUpdateNotification>,
}

impl ClaudeAgentServer {
    pub fn new(config: crate::config::AgentConfig) -> crate::Result<Self> {
        let (agent, notification_receiver) = ClaudeAgent::new(config)?;
        
        Ok(Self {
            agent: Arc::new(agent),
            notification_receiver,
        })
    }
    
    pub async fn start_stdio(&self) -> crate::Result<()> {
        tracing::info!("Starting ACP server on stdio");
        
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        
        self.start_with_streams(stdin, stdout).await
    }
    
    pub async fn start_with_streams<R, W>(&self, reader: R, writer: W) -> crate::Result<()>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        tracing::info!("Starting ACP server with custom streams");
        
        let transport = JsonRpcTransport::new(reader, writer);
        let agent = Arc::clone(&self.agent);
        
        // Start notification sender task
        let notification_task = self.start_notification_sender(&transport).await?;
        
        // Start main request handler
        let request_task = self.start_request_handler(&transport, agent).await?;
        
        // Wait for either task to complete
        tokio::select! {
            result = notification_task => {
                tracing::warn!("Notification task completed: {:?}", result);
            }
            result = request_task => {
                tracing::warn!("Request task completed: {:?}", result);
            }
        }
        
        Ok(())
    }
}
```

### 2. Request Handling

```rust
impl ClaudeAgentServer {
    async fn start_request_handler(
        &self,
        transport: &JsonRpcTransport,
        agent: Arc<ClaudeAgent>,
    ) -> crate::Result<tokio::task::JoinHandle<crate::Result<()>>> {
        let transport_clone = transport.clone();
        
        let task = tokio::spawn(async move {
            loop {
                match transport_clone.receive_request().await {
                    Ok(request) => {
                        let agent_clone = Arc::clone(&agent);
                        let transport_clone2 = transport_clone.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_request(request, agent_clone, transport_clone2).await {
                                tracing::error!("Request handling error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Failed to receive request: {}", e);
                        break;
                    }
                }
            }
            
            Ok(())
        });
        
        Ok(task)
    }
    
    async fn handle_request(
        request: agent_client_protocol::JsonRpcRequest,
        agent: Arc<ClaudeAgent>,
        transport: JsonRpcTransport,
    ) -> crate::Result<()> {
        use agent_client_protocol::JsonRpcRequest;
        
        match request {
            JsonRpcRequest::Initialize { id, params } => {
                let response = agent.initialize(params).await?;
                transport.send_response(id, response).await?;
            }
            JsonRpcRequest::Authenticate { id, params } => {
                let response = agent.authenticate(params).await?;
                transport.send_response(id, response).await?;
            }
            JsonRpcRequest::SessionNew { id, params } => {
                let response = agent.session_new(params).await?;
                transport.send_response(id, response).await?;
            }
            JsonRpcRequest::SessionPrompt { id, params } => {
                let response = agent.session_prompt(params).await?;
                transport.send_response(id, response).await?;
            }
            JsonRpcRequest::ToolPermissionGrant { id, params } => {
                // TODO: Implement tool permission granting
                let response = agent_client_protocol::ToolPermissionGrantResponse {
                    success: true,
                    error_message: None,
                };
                transport.send_response(id, response).await?;
            }
        }
        
        Ok(())
    }
}
```

### 3. Notification Handling

```rust
impl ClaudeAgentServer {
    async fn start_notification_sender(
        &self,
        transport: &JsonRpcTransport,
    ) -> crate::Result<tokio::task::JoinHandle<crate::Result<()>>> {
        let mut notification_receiver = self.notification_receiver.resubscribe();
        let transport_clone = transport.clone();
        
        let task = tokio::spawn(async move {
            while let Ok(notification) = notification_receiver.recv().await {
                if let Err(e) = transport_clone.send_notification(notification).await {
                    tracing::error!("Failed to send notification: {}", e);
                }
            }
            
            Ok(())
        });
        
        Ok(task)
    }
}
```

### 4. Connection Management

```rust
pub struct ConnectionManager {
    active_connections: Arc<tokio::sync::RwLock<std::collections::HashMap<String, ConnectionInfo>>>,
}

#[derive(Debug, Clone)]
struct ConnectionInfo {
    id: String,
    created_at: std::time::SystemTime,
    last_activity: std::time::SystemTime,
    protocol_version: agent_client_protocol::ProtocolVersion,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            active_connections: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    pub async fn register_connection(&self, id: String, protocol_version: agent_client_protocol::ProtocolVersion) -> crate::Result<()> {
        let now = std::time::SystemTime::now();
        let info = ConnectionInfo {
            id: id.clone(),
            created_at: now,
            last_activity: now,
            protocol_version,
        };
        
        let mut connections = self.active_connections.write().await;
        connections.insert(id, info);
        
        tracing::info!("Registered new connection");
        Ok(())
    }
    
    pub async fn update_activity(&self, id: &str) -> crate::Result<()> {
        let mut connections = self.active_connections.write().await;
        if let Some(info) = connections.get_mut(id) {
            info.last_activity = std::time::SystemTime::now();
        }
        Ok(())
    }
    
    pub async fn remove_connection(&self, id: &str) -> crate::Result<()> {
        let mut connections = self.active_connections.write().await;
        connections.remove(id);
        
        tracing::info!("Removed connection: {}", id);
        Ok(())
    }
    
    pub async fn list_connections(&self) -> crate::Result<Vec<String>> {
        let connections = self.active_connections.read().await;
        Ok(connections.keys().cloned().collect())
    }
}
```

### 5. Graceful Shutdown

```rust
use tokio::signal;

impl ClaudeAgentServer {
    pub async fn start_with_shutdown(&self) -> crate::Result<()> {
        let shutdown_signal = self.setup_shutdown_handler();
        
        let server_task = self.start_stdio();
        
        tokio::select! {
            result = server_task => {
                tracing::info!("Server task completed: {:?}", result);
                result
            }
            _ = shutdown_signal => {
                tracing::info!("Received shutdown signal, stopping server");
                self.shutdown().await?;
                Ok(())
            }
        }
    }
    
    async fn setup_shutdown_handler(&self) -> crate::Result<()> {
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())?;
            
            tokio::select! {
                _ = sigterm.recv() => {
                    tracing::info!("Received SIGTERM");
                }
                _ = sigint.recv() => {
                    tracing::info!("Received SIGINT");
                }
            }
        }
        
        #[cfg(not(unix))]
        {
            signal::ctrl_c().await?;
            tracing::info!("Received Ctrl+C");
        }
        
        Ok(())
    }
    
    async fn shutdown(&self) -> crate::Result<()> {
        tracing::info!("Shutting down server");
        
        // Clean up active sessions
        // Stop notification tasks
        // Close connections gracefully
        
        tracing::info!("Server shutdown complete");
        Ok(())
    }
}
```

### 6. Error Recovery

```rust
impl ClaudeAgentServer {
    async fn handle_connection_error(&self, error: &crate::AgentError) -> bool {
        match error {
            crate::AgentError::Protocol(_) => {
                tracing::error!("Protocol error, connection should be closed: {}", error);
                false // Don't retry
            }
            crate::AgentError::Claude(_) => {
                tracing::warn!("Claude API error, retrying: {}", error);
                true // Retry
            }
            crate::AgentError::Io(_) => {
                tracing::error!("IO error, connection lost: {}", error);
                false // Don't retry
            }
            _ => {
                tracing::error!("Unknown error: {}", error);
                false // Don't retry by default
            }
        }
    }
    
    async fn retry_with_backoff<F, T>(&self, mut operation: F, max_attempts: usize) -> crate::Result<T>
    where
        F: FnMut() -> futures::future::BoxFuture<'_, crate::Result<T>>,
    {
        let mut attempts = 0;
        
        loop {
            attempts += 1;
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if attempts < max_attempts && self.handle_connection_error(&e).await => {
                    let delay = std::time::Duration::from_millis(100 * 2_u64.pow(attempts as u32 - 1));
                    tracing::info!("Retrying in {:?} (attempt {} of {})", delay, attempts, max_attempts);
                    tokio::time::sleep(delay).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

### 7. Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, DuplexStream};
    
    async fn create_test_server() -> (ClaudeAgentServer, DuplexStream, DuplexStream) {
        let config = crate::config::AgentConfig::default();
        let server = ClaudeAgentServer::new(config).unwrap();
        
        let (client_stream, server_stream) = duplex(1024);
        
        (server, client_stream, server_stream)
    }
    
    #[tokio::test]
    async fn test_server_creation() {
        let config = crate::config::AgentConfig::default();
        let server = ClaudeAgentServer::new(config);
        assert!(server.is_ok());
    }
    
    #[tokio::test]
    async fn test_connection_manager() {
        let manager = ConnectionManager::new();
        
        manager.register_connection(
            "test-conn".to_string(),
            agent_client_protocol::ProtocolVersion::V1_0_0,
        ).await.unwrap();
        
        let connections = manager.list_connections().await.unwrap();
        assert_eq!(connections.len(), 1);
        assert!(connections.contains(&"test-conn".to_string()));
        
        manager.remove_connection("test-conn").await.unwrap();
        
        let connections = manager.list_connections().await.unwrap();
        assert!(connections.is_empty());
    }
    
    #[tokio::test]
    async fn test_stream_server() {
        let (server, client_stream, server_stream) = create_test_server().await;
        
        // Test that server can be started with custom streams
        let server_task = tokio::spawn(async move {
            server.start_with_streams(server_stream, client_stream).await
        });
        
        // Give server time to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        // For this test, we just verify it starts without errors
        // Full protocol testing will be in integration tests
        
        server_task.abort();
    }
}
```

## Files Created
- `lib/src/server.rs` - ACP server implementation
- Update `lib/src/lib.rs` to export server module

## Acceptance Criteria
- Server can start with stdio transport
- Server supports generic stream interface for testing
- Request handling routes to appropriate agent methods
- Notification sending works for streaming responses
- Connection management tracks active connections
- Graceful shutdown handling works
- Error recovery includes retry logic with backoff
- Unit tests pass for server components
- `cargo build` and `cargo test` succeed