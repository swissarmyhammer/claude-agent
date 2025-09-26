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

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AgentError {
    /// Convert agent error to JSON-RPC error code
    pub fn to_json_rpc_error(&self) -> i32 {
        match self {
            AgentError::Protocol(_) => -32600,         // Invalid Request
            AgentError::MethodNotFound(_) => -32601,   // Method not found
            AgentError::InvalidRequest(_) => -32602,   // Invalid params
            AgentError::Internal(_) => -32603,         // Internal error
            AgentError::PermissionDenied(_) => -32000, // Server error
            AgentError::ToolExecution(_) => -32000,    // Server error
            AgentError::Session(_) => -32000,          // Server error
            AgentError::Config(_) => -32000,           // Server error
            _ => -32603,                               // Internal error (default)
        }
    }
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

        let err = AgentError::PermissionDenied("access denied".to_string());
        assert_eq!(err.to_string(), "Permission denied: access denied");

        let err = AgentError::InvalidRequest("bad request".to_string());
        assert_eq!(err.to_string(), "Invalid request: bad request");

        let err = AgentError::MethodNotFound("unknown method".to_string());
        assert_eq!(err.to_string(), "Method not found: unknown method");

        let err = AgentError::Internal("internal error".to_string());
        assert_eq!(err.to_string(), "Internal error: internal error");
    }

    #[test]
    fn test_json_rpc_error_codes() {
        let err = AgentError::Protocol("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32600);

        let err = AgentError::MethodNotFound("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32601);

        let err = AgentError::InvalidRequest("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32602);

        let err = AgentError::Internal("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32603);

        let err = AgentError::PermissionDenied("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32000);

        let err = AgentError::ToolExecution("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32000);

        let err = AgentError::Session("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32000);

        let err = AgentError::Config("test".to_string());
        assert_eq!(err.to_json_rpc_error(), -32000);
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

        // Test successful result
        if let Ok(value) = success {
            assert_eq!(value, 42);
        }

        // Test error result
        if let Err(error) = failure {
            assert!(matches!(error, AgentError::Protocol(_)));
        }
    }
}
