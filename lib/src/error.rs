//! Error types for the Claude Agent

use thiserror::Error;

/// Main error type for the Claude Agent
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Claude SDK error: {0}")]
    Claude(#[from] claude_sdk_rs::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Convenience type alias for Results using AgentError
pub type Result<T> = std::result::Result<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_display() {
        let err = AgentError::Protocol("test protocol error".to_string());
        assert_eq!(err.to_string(), "Protocol error: test protocol error");

        let err = AgentError::Session("session timeout".to_string());
        assert_eq!(err.to_string(), "Session error: session timeout");

        let err = AgentError::ToolExecution("tool failed".to_string());
        assert_eq!(err.to_string(), "Tool execution error: tool failed");

        let err = AgentError::Config("invalid config".to_string());
        assert_eq!(err.to_string(), "Configuration error: invalid config");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let agent_error: AgentError = io_error.into();

        match agent_error {
            AgentError::Io(_) => {} // Expected
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_serde_error_conversion() {
        let json = "{invalid json";
        let serde_error = serde_json::from_str::<serde_json::Value>(json).unwrap_err();
        let agent_error: AgentError = serde_error.into();

        match agent_error {
            AgentError::Serialization(_) => {} // Expected
            _ => panic!("Expected Serialization variant"),
        }
    }

    #[test]
    fn test_result_type_alias() {
        let success: Result<i32> = Ok(42);
        let failure: Result<i32> = Err(AgentError::Protocol("test".to_string()));

        assert!(success.is_ok());
        assert!(failure.is_err());
        assert_eq!(success.expect("success should be Ok"), 42);
        assert!(matches!(failure.expect_err("failure should be Err"), AgentError::Protocol(_)));
    }
}
