//! Configuration types for the Claude Agent

use serde::{Deserialize, Serialize};

/// Default value for max_prompt_length
fn default_max_prompt_length() -> usize {
    100_000
}

/// Main configuration structure for the Claude Agent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub claude: ClaudeConfig,
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub mcp_servers: Vec<McpServerConfig>,
    /// Maximum allowed prompt length in characters (default: 100,000)
    #[serde(default = "default_max_prompt_length")]
    pub max_prompt_length: usize,
}

/// Configuration for Claude SDK integration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeConfig {
    pub model: String,
    pub stream_format: StreamFormat,
}

/// Server configuration options  
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub port: Option<u16>,
    pub log_level: String,
}

/// Security configuration options
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub allowed_file_patterns: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub require_permission_for: Vec<String>,
}

/// Configuration for MCP server connections
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub protocol: McpProtocolConfig,
}

/// MCP protocol configuration settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpProtocolConfig {
    /// MCP protocol version (default: "2024-11-05")
    #[serde(default = "default_mcp_protocol_version")]
    pub version: String,
    /// Connection timeout in seconds (default: 30)
    #[serde(default = "default_mcp_timeout")]
    pub timeout_seconds: u64,
    /// Maximum retries for initialization (default: 3)
    #[serde(default = "default_mcp_max_retries")]
    pub max_retries: u32,
}

fn default_mcp_protocol_version() -> String {
    "2024-11-05".to_string()
}

fn default_mcp_timeout() -> u64 {
    30
}

fn default_mcp_max_retries() -> u32 {
    3
}

impl Default for McpProtocolConfig {
    fn default() -> Self {
        Self {
            version: default_mcp_protocol_version(),
            timeout_seconds: default_mcp_timeout(),
            max_retries: default_mcp_max_retries(),
        }
    }
}

/// Stream format options for Claude responses
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum StreamFormat {
    StreamJson,
    Standard,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            claude: ClaudeConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                stream_format: StreamFormat::StreamJson,
            },
            server: ServerConfig {
                port: None,
                log_level: "info".to_string(),
            },
            security: SecurityConfig {
                allowed_file_patterns: vec![
                    "**/*.rs".to_string(),
                    "**/*.md".to_string(),
                    "**/*.toml".to_string(),
                ],
                forbidden_paths: vec!["/etc".to_string(), "/usr".to_string(), "/bin".to_string()],
                require_permission_for: vec!["fs_write".to_string(), "terminal_create".to_string()],
            },
            mcp_servers: vec![],
            max_prompt_length: 100_000,
        }
    }
}

impl AgentConfig {
    /// Validate the configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        // Validate model name is not empty
        if self.claude.model.is_empty() {
            return Err(crate::error::AgentError::Config(
                "Claude model cannot be empty".to_string(),
            ));
        }

        // Validate log level
        if !["error", "warn", "info", "debug", "trace"].contains(&self.server.log_level.as_str()) {
            return Err(crate::error::AgentError::Config(format!(
                "Invalid log level: {}",
                self.server.log_level
            )));
        }

        // Validate MCP server configurations
        for server in &self.mcp_servers {
            if server.name.is_empty() {
                return Err(crate::error::AgentError::Config(
                    "MCP server name cannot be empty".to_string(),
                ));
            }
            if server.command.is_empty() {
                return Err(crate::error::AgentError::Config(format!(
                    "MCP server '{}' command cannot be empty",
                    server.name
                )));
            }
        }

        Ok(())
    }

    /// Load configuration from JSON string
    pub fn from_json(json: &str) -> crate::error::Result<Self> {
        let config: AgentConfig = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    /// Serialize configuration to JSON string
    pub fn to_json(&self) -> crate::error::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl SecurityConfig {
    /// Convert SecurityConfig to ToolPermissions for tool call handler
    pub fn to_tool_permissions(&self) -> crate::tools::ToolPermissions {
        crate::tools::ToolPermissions {
            require_permission_for: self.require_permission_for.clone(),
            auto_approved: vec![], // Can be extended later if needed
            forbidden_paths: self.forbidden_paths.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();

        assert_eq!(config.claude.model, "claude-sonnet-4-20250514");
        assert!(matches!(
            config.claude.stream_format,
            StreamFormat::StreamJson
        ));
        assert_eq!(config.server.port, None);
        assert_eq!(config.server.log_level, "info");
        assert_eq!(config.security.allowed_file_patterns.len(), 3);
        assert_eq!(config.security.forbidden_paths.len(), 3);
        assert_eq!(config.security.require_permission_for.len(), 2);
        assert_eq!(config.mcp_servers.len(), 0);
    }

    #[test]
    fn test_config_validation_success() {
        let config = AgentConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_model() {
        let mut config = AgentConfig::default();
        config.claude.model = String::new();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("model cannot be empty"));
    }

    #[test]
    fn test_config_validation_invalid_log_level() {
        let mut config = AgentConfig::default();
        config.server.log_level = "invalid".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid log level"));
    }

    #[test]
    fn test_config_validation_empty_mcp_server_name() {
        let mut config = AgentConfig::default();
        config.mcp_servers.push(McpServerConfig {
            name: String::new(),
            command: "test".to_string(),
            args: vec![],
            protocol: McpProtocolConfig::default(),
        });

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("name cannot be empty"));
    }

    #[test]
    fn test_config_validation_empty_mcp_server_command() {
        let mut config = AgentConfig::default();
        config.mcp_servers.push(McpServerConfig {
            name: "test".to_string(),
            command: String::new(),
            args: vec![],
            protocol: McpProtocolConfig::default(),
        });

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("command cannot be empty"));
    }

    #[test]
    fn test_json_serialization() {
        let config = AgentConfig::default();
        let json = config.to_json().unwrap();

        // Should be valid JSON
        assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok());

        // Should contain expected fields
        assert!(json.contains("claude"));
        assert!(json.contains("server"));
        assert!(json.contains("security"));
        assert!(json.contains("mcp_servers"));
    }

    #[test]
    fn test_json_deserialization() {
        let json = r#"{
            "claude": {
                "model": "test-model",
                "stream_format": "Standard"
            },
            "server": {
                "port": 8080,
                "log_level": "debug"
            },
            "security": {
                "allowed_file_patterns": ["**/*.txt"],
                "forbidden_paths": ["/tmp"],
                "require_permission_for": ["test"]
            },
            "mcp_servers": [
                {
                    "name": "test-server",
                    "command": "test-command",
                    "args": ["--test"]
                }
            ]
        }"#;

        let config = AgentConfig::from_json(json).unwrap();

        assert_eq!(config.claude.model, "test-model");
        assert!(matches!(
            config.claude.stream_format,
            StreamFormat::Standard
        ));
        assert_eq!(config.server.port, Some(8080));
        assert_eq!(config.server.log_level, "debug");
        assert_eq!(config.security.allowed_file_patterns, vec!["**/*.txt"]);
        assert_eq!(config.security.forbidden_paths, vec!["/tmp"]);
        assert_eq!(config.security.require_permission_for, vec!["test"]);
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.mcp_servers[0].name, "test-server");
        assert_eq!(config.mcp_servers[0].command, "test-command");
        assert_eq!(config.mcp_servers[0].args, vec!["--test"]);
        assert_eq!(config.max_prompt_length, 100_000); // Should use default value
    }

    #[test]
    fn test_round_trip_serialization() {
        let original = AgentConfig::default();
        let json = original.to_json().unwrap();
        let deserialized = AgentConfig::from_json(&json).unwrap();

        // Should be equivalent after round trip
        assert_eq!(original.claude.model, deserialized.claude.model);
        assert_eq!(original.server.port, deserialized.server.port);
        assert_eq!(original.server.log_level, deserialized.server.log_level);
        assert_eq!(
            original.security.allowed_file_patterns,
            deserialized.security.allowed_file_patterns
        );
        assert_eq!(
            original.security.forbidden_paths,
            deserialized.security.forbidden_paths
        );
        assert_eq!(
            original.security.require_permission_for,
            deserialized.security.require_permission_for
        );
        assert_eq!(original.mcp_servers.len(), deserialized.mcp_servers.len());
    }
}
