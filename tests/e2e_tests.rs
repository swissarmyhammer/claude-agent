//! End-to-End Protocol Tests
//!
//! This module contains comprehensive integration tests for the ACP protocol
//! implementation, testing the complete flow from initialization to prompt responses.

use std::time::Duration;
use tokio::time::timeout;
use futures::future::join_all;
use std::collections::HashSet;

mod test_client;
use test_client::{TestClient, TestServerHandle, Result};

/// Test helper to create and initialize a test setup
async fn create_test_setup() -> Result<(TestClient, TestServerHandle)> {
    let (client, server_handle) = TestClient::new().await?;
    
    // Give server time to start
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    Ok((client, server_handle))
}

/// Initialize a client with default settings
async fn initialize_client(client: &TestClient) -> Result<String> {
    // Initialize protocol
    let init_response = client.initialize(None).await?;
    println!("Initialize response: {:?}", init_response);
    
    // Authenticate 
    let auth_response = client.authenticate("none".to_string()).await?;
    println!("Auth response: {:?}", auth_response);
    
    // Create session
    let session = client.create_session(None).await?;
    println!("Session response: {:?}", session);
    
    Ok(session.session_id)
}

#[tokio::test]
async fn test_complete_session_flow() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    // Test complete protocol flow
    let result = timeout(Duration::from_secs(5), async {
        // Initialize protocol with capabilities
        let capabilities = agent_client_protocol::ClientCapabilities {
            fs: Some(agent_client_protocol::FsCapabilities {
                read_text_file: true,
                write_text_file: false,
            }),
            terminal: Some(true),
        };
        
        let init_response = client.initialize(Some(capabilities.clone())).await?;
        assert_eq!(init_response.protocol_version, agent_client_protocol::ProtocolVersion::V1_0_0);
        
        // Authenticate
        let auth_response = client.authenticate("none".to_string()).await?;
        assert!(auth_response.success);
        
        // Create session  
        let session_response = client.create_session(Some(capabilities)).await?;
        assert!(!session_response.session_id.is_empty());
        
        // Send prompt
        let prompt_response = client.send_prompt(
            session_response.session_id.clone(),
            "Hello, how are you today?".to_string()
        ).await?;
        
        assert_eq!(prompt_response.session_id, session_response.session_id);
        
        Result::Ok(())
    }).await;
    
    // Clean up
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Complete session flow test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test] 
async fn test_protocol_initialization() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(3), async {
        // Test basic initialization
        let init_response = client.initialize(None).await?;
        
        // Verify response structure
        assert_eq!(init_response.protocol_version, agent_client_protocol::ProtocolVersion::V1_0_0);
        assert!(init_response.server_capabilities.is_some());
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Protocol initialization test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test]
async fn test_authentication_flow() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(3), async {
        // Initialize first
        client.initialize(None).await?;
        
        // Test authentication
        let auth_response = client.authenticate("none".to_string()).await?;
        assert!(auth_response.success);
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Authentication flow test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e), 
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test]
async fn test_session_creation() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(3), async {
        // Initialize and authenticate first
        client.initialize(None).await?;
        client.authenticate("none".to_string()).await?;
        
        // Test session creation
        let session_response = client.create_session(None).await?;
        
        // Validate session ID format (should be a valid UUID or similar)
        assert!(!session_response.session_id.is_empty());
        assert!(session_response.session_id.len() > 10); // Basic sanity check
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Session creation test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test]
async fn test_prompt_handling() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(5), async {
        // Full setup
        let session_id = initialize_client(&client).await?;
        
        // Send a simple prompt
        let prompt_response = client.send_prompt(
            session_id.clone(),
            "What is 2 + 2?".to_string()
        ).await?;
        
        // Validate response
        assert_eq!(prompt_response.session_id, session_id);
        // Note: We can't easily validate the actual content without a real Claude backend
        // But we can validate that the protocol worked correctly
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Prompt handling test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test]
async fn test_multiple_sessions() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(10), async {
        // Initialize once
        client.initialize(None).await?;
        client.authenticate("none".to_string()).await?;
        
        // Create multiple sessions
        let mut session_ids = Vec::new();
        
        for _i in 0..3 {
            let session = client.create_session(None).await?;
            session_ids.push(session.session_id);
        }
        
        // All session IDs should be unique
        let unique_sessions: HashSet<_> = session_ids.iter().collect();
        assert_eq!(unique_sessions.len(), 3);
        
        // Test that each session can handle prompts independently
        for (i, session_id) in session_ids.iter().enumerate() {
            let response = client.send_prompt(
                session_id.clone(),
                format!("Test prompt {}", i + 1)
            ).await?;
            
            assert_eq!(&response.session_id, session_id);
        }
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Multiple sessions test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test]
async fn test_error_handling() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(5), async {
        // Test calling methods without proper initialization sequence
        
        // Try to create session without auth - should fail
        let result = client.create_session(None).await;
        assert!(result.is_err(), "Should fail without authentication");
        
        // Initialize but don't authenticate
        client.initialize(None).await?;
        
        // Try to create session without auth - should still fail  
        let result = client.create_session(None).await;
        assert!(result.is_err(), "Should fail without authentication");
        
        // Now do proper auth
        client.authenticate("none".to_string()).await?;
        
        // Now session creation should work
        let session = client.create_session(None).await?;
        assert!(!session.session_id.is_empty());
        
        // Test invalid session ID
        let result = client.send_prompt(
            "invalid-session-id".to_string(),
            "Hello".to_string()
        ).await;
        assert!(result.is_err(), "Should fail with invalid session ID");
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Error handling test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test] 
async fn test_concurrent_requests() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(10), async {
        // Setup
        let session_id = initialize_client(&client).await?;
        
        // Create multiple concurrent prompt requests
        let tasks: Vec<_> = (0..5).map(|i| {
            let session_id = session_id.clone();
            let client = &client;
            async move {
                client.send_prompt(
                    session_id,
                    format!("Concurrent test {}", i)
                ).await
            }
        }).collect();
        
        // Execute all requests concurrently
        let results = join_all(tasks).await;
        
        // All requests should succeed
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(response) => {
                    assert_eq!(response.session_id, session_id);
                    println!("Concurrent request {} succeeded", i);
                }
                Err(e) => panic!("Concurrent request {} failed: {}", i, e),
            }
        }
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Concurrent requests test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test]
async fn test_session_isolation() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(10), async {
        // Setup
        client.initialize(None).await?;
        client.authenticate("none".to_string()).await?;
        
        // Create two separate sessions
        let session1 = client.create_session(None).await?;
        let session2 = client.create_session(None).await?;
        
        // Ensure sessions have different IDs
        assert_ne!(session1.session_id, session2.session_id);
        
        // Send different prompts to each session
        let response1 = client.send_prompt(
            session1.session_id.clone(),
            "Session 1 test message".to_string()
        ).await?;
        
        let response2 = client.send_prompt(
            session2.session_id.clone(),
            "Session 2 test message".to_string() 
        ).await?;
        
        // Responses should match their respective sessions
        assert_eq!(response1.session_id, session1.session_id);
        assert_eq!(response2.session_id, session2.session_id);
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Session isolation test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

#[tokio::test]
async fn test_capabilities_negotiation() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(5), async {
        // Test with specific client capabilities
        let client_capabilities = agent_client_protocol::ClientCapabilities {
            fs: Some(agent_client_protocol::FsCapabilities {
                read_text_file: true,
                write_text_file: true,
            }),
            terminal: Some(true),
        };
        
        let init_response = client.initialize(Some(client_capabilities)).await?;
        
        // Server should respond with its capabilities
        assert!(init_response.server_capabilities.is_some());
        
        let server_caps = init_response.server_capabilities.unwrap();
        
        // Validate some expected server capabilities exist
        // (The exact structure depends on the server implementation)
        println!("Server capabilities: {:?}", server_caps);
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Capabilities negotiation test passed"),
        Ok(Err(e)) => panic!("Test failed: {}", e),
        Err(_) => panic!("Test timed out"),
    }
}

/// Helper function to validate session ID format
fn assert_valid_session_id(session_id: &str) {
    assert!(!session_id.is_empty(), "Session ID cannot be empty");
    assert!(session_id.len() >= 8, "Session ID should be at least 8 characters");
    // Could add more specific validation based on the ID format used
}

/// Integration test that combines multiple aspects
#[tokio::test]
async fn test_full_integration() {
    let (client, server_handle) = create_test_setup().await.unwrap();
    
    let result = timeout(Duration::from_secs(15), async {
        // Full integration test combining multiple features
        
        // 1. Initialize with full capabilities
        let capabilities = agent_client_protocol::ClientCapabilities {
            fs: Some(agent_client_protocol::FsCapabilities {
                read_text_file: true,
                write_text_file: false,
            }),
            terminal: Some(true),
        };
        
        let init_response = client.initialize(Some(capabilities.clone())).await?;
        assert_eq!(init_response.protocol_version, agent_client_protocol::ProtocolVersion::V1_0_0);
        
        // 2. Authenticate
        let auth_response = client.authenticate("none".to_string()).await?;
        assert!(auth_response.success);
        
        // 3. Create multiple sessions and test them
        let mut sessions = Vec::new();
        for _i in 0..3 {
            let session = client.create_session(Some(capabilities.clone())).await?;
            assert_valid_session_id(&session.session_id);
            sessions.push(session.session_id);
        }
        
        // 4. Send prompts to each session
        for (i, session_id) in sessions.iter().enumerate() {
            let response = client.send_prompt(
                session_id.clone(),
                format!("Integration test prompt #{}", i + 1)
            ).await?;
            
            assert_eq!(&response.session_id, session_id);
        }
        
        // 5. Test error conditions
        let error_result = client.send_prompt(
            "nonexistent-session".to_string(),
            "This should fail".to_string()
        ).await;
        assert!(error_result.is_err(), "Should fail with invalid session");
        
        Result::Ok(())
    }).await;
    
    server_handle.abort();
    
    match result {
        Ok(Ok(())) => println!("Full integration test passed"),
        Ok(Err(e)) => panic!("Integration test failed: {}", e),
        Err(_) => panic!("Integration test timed out"),
    }
}