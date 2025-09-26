//! Claude Agent Library
//!
//! A Rust library that implements an Agent Client Protocol (ACP) server,
//! wrapping Claude Code functionality to enable any ACP-compatible client
//! to interact with Claude Code.

use anyhow::Result;

/// Core configuration for the Claude Agent
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Configuration for Claude integration
    pub claude_config: ClaudeConfig,
    /// Server configuration
    pub server_config: ServerConfig,
}

/// Configuration for Claude SDK integration
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    /// The Claude model to use
    pub model: String,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server port (if using TCP mode)
    pub port: Option<u16>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            claude_config: ClaudeConfig {
                model: "claude-sonnet-4-20250514".to_string(),
            },
            server_config: ServerConfig { port: None },
        }
    }
}

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
            self.config.claude_config.model
        );
        
        // Basic ACP server implementation using stdio
        tracing::info!("ACP server listening on stdio for requests");
        
        // Read from stdin and write to stdout
        use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
        
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();
        
        tracing::info!("Server ready to process ACP messages");
        
        // Simple message loop for ACP protocol
        while let Some(line) = lines.next_line().await? {
            tracing::debug!("Received: {}", line);
            
            // For now, echo back a basic ACP response
            // In a full implementation, this would parse the ACP message
            // and route to appropriate Claude Code functionality
            let response = format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"processed: {}\"}}\n", line);
            stdout.write_all(response.as_bytes()).await?;
            stdout.flush().await?;
            
            tracing::debug!("Sent response");
        }
        
        tracing::info!("ACP server shutting down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();
        assert_eq!(config.claude_config.model, "claude-sonnet-4-20250514");
        assert_eq!(config.server_config.port, None);
    }

    #[test]
    fn test_server_creation() {
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config.clone());
        // Verify the server has the correct config
        assert_eq!(server.config.claude_config.model, config.claude_config.model);
    }

    #[test]
    fn test_custom_config() {
        let mut config = AgentConfig::default();
        config.claude_config.model = "custom-model".to_string();
        config.server_config.port = Some(8080);
        
        let server = ClaudeAgentServer::new(config);
        assert_eq!(server.config.claude_config.model, "custom-model");
        assert_eq!(server.config.server_config.port, Some(8080));
    }

    #[test]
    fn test_config_clone() {
        let config1 = AgentConfig::default();
        let config2 = config1.clone();
        assert_eq!(config1.claude_config.model, config2.claude_config.model);
        assert_eq!(config1.server_config.port, config2.server_config.port);
    }

    #[tokio::test]
    async fn test_server_creation_async() {
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config);
        
        // Test that the server can be created without panic
        // Note: We can't easily test start_stdio() as it reads from stdin
        // which would require complex mocking in a unit test environment
        assert_eq!(server.config.claude_config.model, "claude-sonnet-4-20250514");
    }
}