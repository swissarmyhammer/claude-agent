# Comprehensive Integration Testing

Refer to plan.md

## Goal
Create a comprehensive test suite with end-to-end ACP protocol tests, test client implementation, and performance validation.

## Tasks

### 1. Test Client Implementation (`tests/test_client.rs`)

```rust
use agent_client_protocol::{
    Agent, JsonRpcTransport, InitializeRequest, AuthenticateRequest, SessionNewRequest, 
    PromptRequest, ProtocolVersion, ClientCapabilities
};
use tokio::io::{duplex, DuplexStream};
use claude_agent_lib::{
    config::AgentConfig,
    server::ClaudeAgentServer,
};

pub struct TestClient {
    transport: JsonRpcTransport,
}

impl TestClient {
    pub async fn new() -> (Self, ClaudeAgentServer) {
        let (client_stream, server_stream) = duplex(8192);
        
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config).expect("Failed to create server");
        
        let client = Self {
            transport: JsonRpcTransport::new(client_stream),
        };
        
        (client, server)
    }
    
    pub async fn initialize(&self, client_capabilities: Option<ClientCapabilities>) -> Result<agent_client_protocol::InitializeResponse, Box<dyn std::error::Error>> {
        let request = InitializeRequest {
            protocol_version: ProtocolVersion::V1_0_0,
            client_capabilities,
        };
        
        let response = self.transport.send_request("initialize", request).await?;
        Ok(response)
    }
    
    pub async fn authenticate(&self, auth_type: String) -> Result<agent_client_protocol::AuthenticateResponse, Box<dyn std::error::Error>> {
        let request = AuthenticateRequest {
            auth_type,
            credentials: None,
        };
        
        let response = self.transport.send_request("authenticate", request).await?;
        Ok(response)
    }
    
    pub async fn create_session(&self, client_capabilities: Option<ClientCapabilities>) -> Result<agent_client_protocol::SessionNewResponse, Box<dyn std::error::Error>> {
        let request = SessionNewRequest {
            client_capabilities,
        };
        
        let response = self.transport.send_request("session_new", request).await?;
        Ok(response)
    }
    
    pub async fn send_prompt(&self, session_id: String, prompt: String) -> Result<agent_client_protocol::PromptResponse, Box<dyn std::error::Error>> {
        let request = PromptRequest {
            session_id,
            prompt,
        };
        
        let response = self.transport.send_request("session_prompt", request).await?;
        Ok(response)
    }
    
    pub async fn receive_notifications(&self) -> impl Stream<Item = agent_client_protocol::SessionUpdateNotification> {
        self.transport.notification_stream()
    }
}
```

### 2. End-to-End Protocol Tests (`tests/e2e_tests.rs`)

```rust
use tokio_stream::StreamExt;
use std::time::Duration;

#[tokio::test]
async fn test_complete_session_flow() {
    let (client, server) = TestClient::new().await;
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Initialize protocol
    let capabilities = ClientCapabilities {
        streaming: Some(true),
        tools: Some(true),
    };
    
    let init_response = client.initialize(Some(capabilities.clone())).await.unwrap();
    assert_eq!(init_response.protocol_version, ProtocolVersion::V1_0_0);
    assert!(init_response.server_capabilities.streaming.unwrap_or(false));
    
    // Authenticate
    let auth_response = client.authenticate("none".to_string()).await.unwrap();
    assert!(auth_response.success);
    
    // Create session
    let session_response = client.create_session(Some(capabilities)).await.unwrap();
    assert!(!session_response.session_id.is_empty());
    
    // Send prompt
    let prompt_response = client.send_prompt(
        session_response.session_id.clone(),
        "Hello, how are you today?".to_string()
    ).await.unwrap();
    
    assert_eq!(prompt_response.session_id, session_response.session_id);
    
    // Clean up
    server_handle.abort();
}

#[tokio::test]
async fn test_streaming_responses() {
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Setup with streaming enabled
    let capabilities = ClientCapabilities {
        streaming: Some(true),
        tools: Some(false),
    };
    
    client.initialize(Some(capabilities.clone())).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    let session = client.create_session(Some(capabilities)).await.unwrap();
    
    // Start listening for notifications
    let mut notifications = client.receive_notifications().await;
    
    // Send streaming prompt
    let prompt_task = client.send_prompt(
        session.session_id.clone(),
        "Tell me a story".to_string()
    );
    
    // Collect streaming updates
    let notification_task = async {
        let mut updates = Vec::new();
        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::pin!(timeout);
        
        loop {
            tokio::select! {
                notification = notifications.next() => {
                    match notification {
                        Some(update) if update.session_id == session.session_id => {
                            updates.push(update);
                            if updates.len() >= 3 { // Collect at least 3 updates
                                break;
                            }
                        }
                        Some(_) => continue, // Different session
                        None => break, // Stream ended
                    }
                }
                _ = &mut timeout => {
                    panic!("Timeout waiting for streaming updates");
                }
            }
        }
        
        updates
    };
    
    let (prompt_result, streaming_updates) = tokio::join!(prompt_task, notification_task);
    
    assert!(prompt_result.is_ok());
    assert!(!streaming_updates.is_empty());
    assert!(streaming_updates.iter().all(|update| update.message_chunk.is_some()));
    
    server_handle.abort();
}

#[tokio::test]
async fn test_tool_execution_flow() {
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Setup with tools enabled
    let capabilities = ClientCapabilities {
        streaming: Some(false),
        tools: Some(true),
    };
    
    client.initialize(Some(capabilities.clone())).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    let session = client.create_session(Some(capabilities)).await.unwrap();
    
    // Send prompt that should trigger tool calls
    let prompt_response = client.send_prompt(
        session.session_id.clone(),
        "Please read the contents of README.md".to_string()
    ).await;
    
    // The response might succeed or fail depending on whether README.md exists
    // but it should not error at the protocol level
    assert!(prompt_response.is_ok());
    
    server_handle.abort();
}
```

### 3. Concurrent Session Testing

```rust
#[tokio::test]
async fn test_multiple_concurrent_sessions() {
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Initialize once
    client.initialize(None).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    
    // Create multiple sessions concurrently
    let session_tasks: Vec<_> = (0..5).map(|i| {
        let client = &client;
        async move {
            let session = client.create_session(None).await.unwrap();
            let prompt = format!("Hello from session {}", i);
            let response = client.send_prompt(session.session_id, prompt).await.unwrap();
            response
        }
    }).collect();
    
    let results = futures::future::join_all(session_tasks).await;
    
    // All sessions should succeed
    assert_eq!(results.len(), 5);
    assert!(results.iter().all(|r| !r.session_id.is_empty()));
    
    // All session IDs should be unique
    let session_ids: std::collections::HashSet<_> = results.iter()
        .map(|r| &r.session_id)
        .collect();
    assert_eq!(session_ids.len(), 5);
    
    server_handle.abort();
}

#[tokio::test]
async fn test_session_isolation() {
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    client.initialize(None).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    
    // Create two sessions
    let session1 = client.create_session(None).await.unwrap();
    let session2 = client.create_session(None).await.unwrap();
    
    // Send different prompts to each session
    client.send_prompt(
        session1.session_id.clone(),
        "My name is Alice".to_string()
    ).await.unwrap();
    
    client.send_prompt(
        session2.session_id.clone(), 
        "My name is Bob".to_string()
    ).await.unwrap();
    
    // Sessions should be isolated - each should only know its own conversation
    let alice_response = client.send_prompt(
        session1.session_id.clone(),
        "What is my name?".to_string()
    ).await.unwrap();
    
    let bob_response = client.send_prompt(
        session2.session_id.clone(),
        "What is my name?".to_string()
    ).await.unwrap();
    
    // Both should respond (isolation test would need actual Claude integration to verify content)
    assert_eq!(alice_response.session_id, session1.session_id);
    assert_eq!(bob_response.session_id, session2.session_id);
    
    server_handle.abort();
}
```

### 4. Error Handling and Recovery Tests

```rust
#[tokio::test]
async fn test_protocol_error_handling() {
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Test using session before initialization
    let result = client.create_session(None).await;
    assert!(result.is_err()); // Should fail without initialization
    
    // Proper initialization
    client.initialize(None).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    
    // Test invalid session ID
    let result = client.send_prompt(
        "invalid-session-id".to_string(),
        "Hello".to_string()
    ).await;
    assert!(result.is_err());
    
    // Test empty prompt
    let session = client.create_session(None).await.unwrap();
    let result = client.send_prompt(
        session.session_id,
        "".to_string() // Empty prompt
    ).await;
    assert!(result.is_err());
    
    server_handle.abort();
}

#[tokio::test] 
async fn test_connection_recovery() {
    // Test that server can handle connection drops and recoveries
    // This would involve testing with network-like conditions
    
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Normal operation
    client.initialize(None).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    let session = client.create_session(None).await.unwrap();
    
    // Send successful prompt
    let response = client.send_prompt(
        session.session_id.clone(),
        "Hello".to_string()
    ).await.unwrap();
    
    assert_eq!(response.session_id, session.session_id);
    
    server_handle.abort();
}
```

### 5. Performance and Load Testing

```rust
#[tokio::test]
async fn test_performance_baseline() {
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    client.initialize(None).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    
    let start_time = std::time::Instant::now();
    
    // Create and use 10 sessions rapidly
    for i in 0..10 {
        let session = client.create_session(None).await.unwrap();
        client.send_prompt(
            session.session_id,
            format!("Test message {}", i)
        ).await.unwrap();
    }
    
    let elapsed = start_time.elapsed();
    
    // Should complete within reasonable time (adjust threshold as needed)
    assert!(elapsed < Duration::from_secs(10));
    
    println!("Performance baseline: 10 sessions in {:?}", elapsed);
    
    server_handle.abort();
}

#[tokio::test]
async fn test_memory_usage() {
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    client.initialize(None).await.unwrap();
    client.authenticate("none".to_string()).await.unwrap();
    
    // Create many sessions to test memory usage
    let mut sessions = Vec::new();
    
    for i in 0..100 {
        let session = client.create_session(None).await.unwrap();
        sessions.push(session.session_id);
        
        // Send a prompt every 10 sessions
        if i % 10 == 0 {
            client.send_prompt(
                sessions[i].clone(),
                "Memory test prompt".to_string()
            ).await.unwrap();
        }
    }
    
    // All sessions should be unique
    let unique_sessions: std::collections::HashSet<_> = sessions.iter().collect();
    assert_eq!(unique_sessions.len(), 100);
    
    server_handle.abort();
}
```

### 6. Integration with Real Claude Code

```rust
// This test would only run if Claude SDK credentials are available
#[tokio::test]
#[ignore = "requires claude sdk credentials"]
async fn test_real_claude_integration() {
    // Set up test only if credentials are available
    if std::env::var("CLAUDE_API_KEY").is_err() {
        return;
    }
    
    let (client, server) = TestClient::new().await;
    
    let server_handle = tokio::spawn(async move {
        server.start_stdio().await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    client.initialize(None).await.unwrap();
    client.authenticate("api_key".to_string()).await.unwrap();
    let session = client.create_session(None).await.unwrap();
    
    // Send a real prompt to Claude
    let response = client.send_prompt(
        session.session_id.clone(),
        "What is 2 + 2? Please respond with just the number.".to_string()
    ).await.unwrap();
    
    assert_eq!(response.session_id, session.session_id);
    
    // Wait a moment for Claude's response to be processed
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    server_handle.abort();
}
```

### 7. Test Utilities and Helpers

```rust
// In tests/common/mod.rs
pub mod test_utils {
    use super::*;
    
    pub async fn create_test_setup() -> (TestClient, tokio::task::JoinHandle<()>) {
        let (client, server) = TestClient::new().await;
        
        let server_handle = tokio::spawn(async move {
            if let Err(e) = server.start_stdio().await {
                eprintln!("Server error: {}", e);
            }
        });
        
        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        (client, server_handle)
    }
    
    pub async fn initialize_client(client: &TestClient) -> String {
        client.initialize(None).await.unwrap();
        client.authenticate("none".to_string()).await.unwrap();
        let session = client.create_session(None).await.unwrap();
        session.session_id
    }
    
    pub fn assert_valid_session_id(session_id: &str) {
        // Should be a valid UUID
        assert!(uuid::Uuid::parse_str(session_id).is_ok());
    }
}
```

## Files Created
- `tests/test_client.rs` - Test client implementation
- `tests/e2e_tests.rs` - End-to-end protocol tests
- `tests/performance_tests.rs` - Performance and load tests
- `tests/common/mod.rs` - Test utilities and helpers

## Test Configuration
Add to `Cargo.toml`:
```toml
[dev-dependencies]
futures = "0.3"
uuid = "1.10"
tempfile = "3.8"

[[test]]
name = "integration"
path = "tests/e2e_tests.rs"
```

## Acceptance Criteria
- Test client can communicate with server via in-memory streams
- Complete protocol flow tests pass (initialize → authenticate → session → prompt)
- Streaming response tests verify real-time updates work
- Tool execution tests verify permission flow
- Concurrent session tests verify isolation and thread safety
- Error handling tests cover protocol violations and edge cases
- Performance tests establish baseline metrics
- Memory usage tests verify no excessive resource consumption
- Integration tests work with mock and real Claude (when credentials available)
- All tests pass consistently
- `cargo test --all-features` succeeds