//! Claude Agent Library
//!
//! A Rust library that implements an Agent Client Protocol (ACP) server,
//! wrapping Claude Code functionality to enable any ACP-compatible client
//! to interact with Claude Code.

pub mod agent;
pub mod claude;
pub mod config;
pub mod error;
pub mod session;
pub mod tools;

pub use agent::ClaudeAgent;
pub use config::AgentConfig;
pub use error::{AgentError, Result};
pub use tools::{ToolCallHandler, ToolCallResult, ToolPermissions};

/// The main Claude Agent ACP server
pub struct ClaudeAgentServer {
    config: AgentConfig,
}

impl ClaudeAgentServer {
    /// Create a new Claude Agent server with the given configuration
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    /// Start the server using stdio (standard ACP pattern)
    pub async fn start_stdio(&self) -> Result<()> {
        tracing::info!(
            "Starting Claude Agent ACP server with model: {}",
            self.config.claude.model
        );

        // Create the Claude Agent that will handle ACP requests
        let _claude_agent = ClaudeAgent::new(self.config.clone())?;

        // TODO: Implement proper ACP server using agent_client_protocol
        // For now, return an error indicating this needs implementation
        tracing::error!("ACP server implementation not yet complete");
        Err(AgentError::ServerError(
            "ACP server implementation requires integration with agent_client_protocol crate".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();
        assert_eq!(config.claude.model, "claude-sonnet-4-20250514");
        assert_eq!(config.server.port, None);
    }

    #[test]
    fn test_server_creation() {
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config.clone());
        // Verify the server has the correct config
        assert_eq!(server.config.claude.model, config.claude.model);
    }

    #[test]
    fn test_custom_config() {
        let mut config = AgentConfig::default();
        config.claude.model = "custom-model".to_string();
        config.server.port = Some(8080);

        let server = ClaudeAgentServer::new(config);
        assert_eq!(server.config.claude.model, "custom-model");
        assert_eq!(server.config.server.port, Some(8080));
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
        let server = ClaudeAgentServer::new(config);

        // Test that the server can be created without panic
        // Note: We can't easily test start_stdio() as it reads from stdin
        // which would require complex mocking in a unit test environment
        assert_eq!(server.config.claude.model, "claude-sonnet-4-20250514");
    }
}
