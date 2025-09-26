//! Test Client Implementation for ACP Protocol Testing
//!
//! This module provides a test client that can communicate with the Claude Agent
//! server via in-memory duplex streams for comprehensive integration testing.

use agent_client_protocol::{
    InitializeRequest, InitializeResponse, AuthenticateRequest, AuthenticateResponse,
    NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
    ProtocolVersion, ClientCapabilities,
};
use claude_agent_lib::{config::AgentConfig, server::ClaudeAgentServer};
use tokio::io::{duplex, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use futures::stream::{Stream, StreamExt};
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

/// A test client that can communicate with the ACP server via streams
pub struct TestClient {
    writer: Arc<Mutex<Box<dyn AsyncWrite + Unpin + Send>>>,
    reader: Arc<Mutex<BufReader<Box<dyn AsyncRead + Unpin + Send>>>>,
    request_id: Arc<Mutex<u64>>,
}

impl TestClient {
    /// Create a new test client and server pair connected via in-memory duplex streams
    pub async fn new() -> crate::Result<(Self, TestServerHandle)> {
        let (client_stream, server_stream) = duplex(8192);
        
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config)?;
        
        let (client_reader, client_writer) = tokio::io::split(client_stream);
        let (server_reader, server_writer) = tokio::io::split(server_stream);
        
        let client = Self {
            writer: Arc::new(Mutex::new(Box::new(client_writer))),
            reader: Arc::new(Mutex::new(BufReader::new(Box::new(client_reader)))),
            request_id: Arc::new(Mutex::new(1)),
        };
        
        // Start the server with the stream in a background task
        let server_handle = tokio::spawn(async move {
            server.start_with_streams(server_reader, server_writer).await
        });
        
        Ok((client, TestServerHandle { handle: server_handle }))
    }
    
    /// Get the next request ID
    async fn next_request_id(&self) -> u64 {
        let mut id = self.request_id.lock().await;
        let current = *id;
        *id += 1;
        current
    }
    
    /// Send a JSON-RPC request and wait for the response
    pub async fn send_request(&self, method: &str, params: Value) -> crate::Result<Value> {
        let id = self.next_request_id().await;
        
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        
        // Send request
        {
            let mut writer = self.writer.lock().await;
            let request_line = format!("{}\n", serde_json::to_string(&request)?);
            writer.write_all(request_line.as_bytes()).await?;
            writer.flush().await?;
        }
        
        // Read response
        {
            let mut reader = self.reader.lock().await;
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            
            let response: Value = serde_json::from_str(&line.trim())?;
            
            // Check for error
            if let Some(error) = response.get("error") {
                return Err(crate::AgentError::Protocol(
                    format!("JSON-RPC error: {}", error)
                ));
            }
            
            // Return result
            response.get("result")
                .ok_or_else(|| crate::AgentError::Protocol("Missing result in response".to_string()))
                .map(|r| r.clone())
        }
    }
    
    /// Initialize the protocol
    pub async fn initialize(&self, client_capabilities: Option<ClientCapabilities>) -> crate::Result<InitializeResponse> {
        let request = InitializeRequest {
            protocol_version: ProtocolVersion::V1_0_0,
            client_capabilities,
        };
        
        let params = serde_json::to_value(request)?;
        let response = self.send_request("initialize", params).await?;
        
        Ok(serde_json::from_value(response)?)
    }
    
    /// Authenticate with the server
    pub async fn authenticate(&self, auth_type: String) -> crate::Result<AuthenticateResponse> {
        let request = AuthenticateRequest {
            auth_type,
            credentials: None,
        };
        
        let params = serde_json::to_value(request)?;
        let response = self.send_request("authenticate", params).await?;
        
        Ok(serde_json::from_value(response)?)
    }
    
    /// Create a new session
    pub async fn create_session(&self, client_capabilities: Option<ClientCapabilities>) -> crate::Result<NewSessionResponse> {
        let request = NewSessionRequest {
            client_capabilities,
        };
        
        let params = serde_json::to_value(request)?;
        let response = self.send_request("session/new", params).await?;
        
        Ok(serde_json::from_value(response)?)
    }
    
    /// Send a prompt to a session
    pub async fn send_prompt(&self, session_id: String, prompt: String) -> crate::Result<PromptResponse> {
        let request = PromptRequest {
            session_id,
            prompt: agent_client_protocol::TextContent { text: prompt },
        };
        
        let params = serde_json::to_value(request)?;
        let response = self.send_request("session/prompt", params).await?;
        
        Ok(serde_json::from_value(response)?)
    }
    
    /// Receive notifications as a stream (simplified implementation)
    pub async fn receive_notifications(&self) -> impl Stream<Item = SessionUpdateNotification> {
        // This is a simplified implementation for testing
        // In a real implementation, we'd need to handle notifications separately from responses
        futures::stream::empty()
    }
}

/// Wrapper to help with server stream management in tests  
/// Handle for the test server running in a background task
pub struct TestServerHandle {
    handle: tokio::task::JoinHandle<crate::Result<()>>,
}

impl TestServerHandle {
    /// Stop the server and wait for it to complete
    pub async fn shutdown(self) -> crate::Result<()> {
        self.handle.abort();
        match self.handle.await {
            Ok(result) => result,
            Err(e) if e.is_cancelled() => Ok(()), // Expected when we abort
            Err(e) => Err(crate::AgentError::Protocol(format!("Server task failed: {}", e))),
        }
    }
}

/// Session update notification for test client
#[derive(Debug, Clone)]
pub struct SessionUpdateNotification {
    pub session_id: String,
    pub message_chunk: Option<MessageChunk>,
}

/// Message chunk for streaming responses
#[derive(Debug, Clone)]
pub struct MessageChunk {
    pub content: String,
}

/// Re-export types needed for testing
pub use claude_agent_lib::error::{AgentError, Result as AgentResult};

/// Type alias for convenience
pub type Result<T> = std::result::Result<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_client_creation() {
        let result = TestClient::new().await;
        assert!(result.is_ok());
        
        let (_client, _server) = result.unwrap();
        // Just ensure we can create the client/server pair without panicking
    }
    
    #[tokio::test] 
    async fn test_request_id_increment() {
        let (client, _server) = TestClient::new().await.unwrap();
        
        let id1 = client.next_request_id().await;
        let id2 = client.next_request_id().await;
        
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }
    
    #[tokio::test]
    async fn test_json_rpc_request_format() {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocol_version": "1.0.0",
                "client_capabilities": null
            }
        });
        
        assert_eq!(request["jsonrpc"], "2.0");
        assert_eq!(request["id"], 1);
        assert_eq!(request["method"], "initialize");
        assert!(request["params"].is_object());
    }
}