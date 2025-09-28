//! Claude Agent Library
//!
//! A Rust library that implements an Agent Client Protocol (ACP) server,
//! wrapping Claude Code functionality to enable any ACP-compatible client
//! to interact with Claude Code.

pub mod agent;
pub mod capability_validation;
pub mod claude;
pub mod config;
pub mod error;
pub mod mcp;
pub mod mcp_error_handling;
pub mod path_validator;
pub mod permissions;
pub mod plan;
pub mod request_validation;
pub mod server;
pub mod session;
pub mod session_errors;
pub mod session_loading;
pub mod session_validation;
pub mod tools;

pub use agent::ClaudeAgent;
pub use config::AgentConfig;
pub use error::{AgentError, Result};
pub use plan::{AgentPlan, PlanEntry, PlanEntryStatus, PlanGenerator, PlanManager, Priority};
pub use server::ClaudeAgentServer;
pub use tools::{ToolCallHandler, ToolCallResult, ToolPermissions};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();
        assert_eq!(config.claude.model, "claude-sonnet-4-20250514");
        assert_eq!(config.server.port, None);
    }

    #[tokio::test]
    async fn test_server_creation() {
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config.clone()).await;
        // Verify the server was created successfully
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_custom_config() {
        let mut config = AgentConfig::default();
        config.claude.model = "custom-model".to_string();
        config.server.port = Some(8080);

        let server = ClaudeAgentServer::new(config).await;
        assert!(server.is_ok());
    }

    #[test]
    fn test_config_clone() {
        let config1 = AgentConfig::default();
        let config2 = config1.clone();
        assert_eq!(config1.claude.model, config2.claude.model);
        assert_eq!(config1.server.port, config2.server.port);
    }

    #[tokio::test]
    async fn test_server_creation_async() {
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config).await;

        // Test that the server can be created without panic
        // Note: We can't easily test start_stdio() as it reads from stdin
        // which would require complex mocking in a unit test environment
        assert!(server.is_ok());
    }
}
