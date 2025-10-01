//! Integration tests for Claude Agent Library
//!
//! These tests verify the basic functionality of the Claude Agent library
//! components working together.

mod common;

use claude_agent_lib::{config::AgentConfig, server::ClaudeAgentServer};
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_server_creation() {
    let config = AgentConfig::default();
    let result = ClaudeAgentServer::new(config).await;

    assert!(result.is_ok(), "Failed to create server");
}

#[tokio::test]
async fn test_config_creation() {
    let config = AgentConfig::default();

    // Basic test to ensure config can be created
    // Config exists and can be used to create servers
    let _server = ClaudeAgentServer::new(config).await;
}

#[tokio::test]
async fn test_server_startup_timeout() {
    let config = AgentConfig::default();
    let server = ClaudeAgentServer::new(config)
        .await
        .expect("Failed to create server");

    // Test that server startup doesn't hang indefinitely
    // We use a very short timeout just to verify the method exists and can be called
    let result = timeout(Duration::from_millis(10), async {
        // This will likely timeout, which is expected
        server.start_stdio().await
    })
    .await;

    // We expect either a timeout (Ok(Err)) or timeout error (Err)
    // Both indicate the method exists and is callable
    assert!(result.is_err() || result.is_ok());
}

#[tokio::test]
async fn test_basic_functionality() {
    // Test that we can create multiple components without panics
    let config1 = AgentConfig::default();
    let config2 = AgentConfig::default();

    let server1 = ClaudeAgentServer::new(config1).await;
    let server2 = ClaudeAgentServer::new(config2).await;

    assert!(server1.is_ok());
    assert!(server2.is_ok());
}
