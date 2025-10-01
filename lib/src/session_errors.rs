//! Comprehensive error handling for ACP session setup operations
//!
//! This module implements the error handling requirements from the ACP specification
//! for session setup operations including session/new and session/load.

use crate::error::ToJsonRpcError;
use agent_client_protocol::SessionId;
use serde_json::Value;
use std::path::PathBuf;
use thiserror::Error;

/// Details for invalid parameter type errors
///
/// This struct is boxed to reduce the size of the SessionSetupError enum
#[derive(Debug, Clone)]
pub struct InvalidParameterTypeDetails {
    pub request_type: String,
    pub parameter_name: String,
    pub expected_type: String,
    pub actual_type: String,
    pub provided_value: Value,
}

/// Comprehensive session setup error types following ACP specification requirements
///
/// Each error type includes structured data for programmatic handling and
/// maps to appropriate JSON-RPC error codes as specified in ACP.
#[derive(Error, Debug, Clone)]
pub enum SessionSetupError {
    // Working directory errors
    #[error("Invalid working directory: path must be absolute")]
    WorkingDirectoryNotAbsolute {
        provided_path: PathBuf,
        requirement: String,
        example: String,
    },

    #[error("Working directory not found: {path}")]
    WorkingDirectoryNotFound { path: PathBuf },

    #[error("Working directory access denied: insufficient permissions")]
    WorkingDirectoryPermissionDenied {
        path: PathBuf,
        required_permissions: Vec<String>,
    },

    #[error("Working directory path contains invalid characters")]
    WorkingDirectoryInvalidPath {
        path: PathBuf,
        invalid_chars: Vec<String>,
    },

    #[error("Working directory is a network path which is not supported")]
    WorkingDirectoryNetworkPath { path: PathBuf, suggestion: String },

    // MCP server connection errors
    #[error("MCP server connection failed: executable not found")]
    McpServerExecutableNotFound {
        server_name: String,
        command: PathBuf,
        suggestion: String,
    },

    #[error("MCP server startup failed: process exited with code {exit_code}")]
    McpServerStartupFailed {
        server_name: String,
        exit_code: i32,
        stderr: String,
        suggestion: String,
    },

    #[error("MCP server connection failed: {error}")]
    McpServerConnectionFailed {
        server_name: String,
        error: String,
        transport_type: String,
    },

    #[error("MCP server authentication failed")]
    McpServerAuthenticationFailed {
        server_name: String,
        transport_type: String,
        details: String,
    },

    #[error("MCP server connection timeout after {timeout_ms}ms")]
    McpServerTimeout {
        server_name: String,
        timeout_ms: u64,
        transport_type: String,
    },

    #[error("MCP server protocol negotiation failed")]
    McpServerProtocolNegotiationFailed {
        server_name: String,
        expected_version: String,
        actual_version: Option<String>,
    },

    // Session loading errors
    #[error("Session not found: sessionId does not exist or has expired")]
    SessionNotFound {
        session_id: SessionId,
        available_sessions: Vec<String>,
    },

    #[error("Session expired: session has exceeded maximum age")]
    SessionExpired {
        session_id: SessionId,
        expired_at: String,
        max_age_seconds: u64,
    },

    #[error("Session load failed: corrupted session data")]
    SessionCorrupted {
        session_id: SessionId,
        corruption_details: String,
    },

    #[error("Session storage backend failure")]
    SessionStorageFailure {
        session_id: Option<SessionId>,
        storage_error: String,
        recovery_suggestion: String,
    },

    #[error("Session history replay failed")]
    SessionHistoryReplayFailed {
        session_id: SessionId,
        failed_at_message: usize,
        total_messages: usize,
        error_details: String,
    },

    // Capability validation errors
    #[error("Transport not supported: agent does not support {requested_transport} transport")]
    TransportNotSupported {
        requested_transport: String,
        declared_capability: bool,
        supported_transports: Vec<String>,
    },

    #[error("LoadSession capability not supported")]
    LoadSessionNotSupported { declared_capability: bool },

    #[error("Capability format validation failed")]
    CapabilityFormatError {
        capability_name: String,
        expected_format: String,
        actual_value: Value,
    },

    #[error("Unknown capability: {capability_name}")]
    UnknownCapability {
        capability_name: String,
        known_capabilities: Vec<String>,
    },

    #[error("Capability not supported: {capability_name} required for {required_for}")]
    CapabilityNotSupported {
        capability_name: String,
        required_for: String,
    },

    // Request validation errors
    #[error("Malformed session request: {details}")]
    MalformedRequest {
        request_type: String, // "session/new" or "session/load"
        details: String,
        example: Option<String>,
    },

    #[error("Invalid session ID format")]
    InvalidSessionId {
        provided_id: String,
        expected_format: String,
        example: String,
    },

    #[error("Missing required parameter: {parameter_name}")]
    MissingRequiredParameter {
        request_type: String,
        parameter_name: String,
        parameter_type: String,
    },

    #[error("Invalid parameter type for parameter")]
    InvalidParameterType(Box<InvalidParameterTypeDetails>),

    // Cleanup and recovery errors
    #[error("Partial session cleanup failed")]
    PartialSessionCleanupFailed {
        session_id: SessionId,
        cleanup_errors: Vec<String>,
        resources_not_cleaned: Vec<String>,
    },

    #[error("MCP server cleanup failed")]
    McpServerCleanupFailed {
        server_name: String,
        cleanup_error: String,
    },
}

impl ToJsonRpcError for SessionSetupError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            // Invalid Request (-32602): Invalid method parameter(s)
            Self::WorkingDirectoryNotAbsolute { .. }
            | Self::WorkingDirectoryInvalidPath { .. }
            | Self::TransportNotSupported { .. }
            | Self::LoadSessionNotSupported { .. }
            | Self::CapabilityFormatError { .. }
            | Self::MalformedRequest { .. }
            | Self::InvalidSessionId { .. }
            | Self::MissingRequiredParameter { .. }
            | Self::InvalidParameterType(..)
            | Self::SessionNotFound { .. }
            | Self::UnknownCapability { .. }
            | Self::CapabilityNotSupported { .. } => -32602,

            // Internal Error (-32603): Internal JSON-RPC error
            Self::WorkingDirectoryNotFound { .. }
            | Self::WorkingDirectoryPermissionDenied { .. }
            | Self::WorkingDirectoryNetworkPath { .. }
            | Self::McpServerExecutableNotFound { .. }
            | Self::McpServerStartupFailed { .. }
            | Self::McpServerConnectionFailed { .. }
            | Self::McpServerAuthenticationFailed { .. }
            | Self::McpServerTimeout { .. }
            | Self::McpServerProtocolNegotiationFailed { .. }
            | Self::SessionExpired { .. }
            | Self::SessionCorrupted { .. }
            | Self::SessionStorageFailure { .. }
            | Self::SessionHistoryReplayFailed { .. }
            | Self::PartialSessionCleanupFailed { .. }
            | Self::McpServerCleanupFailed { .. } => -32603,
        }
    }

    fn to_error_data(&self) -> Option<Value> {
        Some(self.to_error_data_internal())
    }
}

impl SessionSetupError {
    /// Convert error to structured data for JSON-RPC error response
    fn to_error_data_internal(&self) -> Value {
        match self {
            Self::WorkingDirectoryNotAbsolute {
                provided_path,
                requirement,
                example,
            } => {
                serde_json::json!({
                    "providedPath": provided_path,
                    "requirement": requirement,
                    "example": example
                })
            }

            Self::WorkingDirectoryNotFound { path } => {
                serde_json::json!({
                    "path": path,
                    "error": "directory_not_found"
                })
            }

            Self::WorkingDirectoryPermissionDenied {
                path,
                required_permissions,
            } => {
                serde_json::json!({
                    "path": path,
                    "error": "permission_denied",
                    "requiredPermissions": required_permissions
                })
            }

            Self::WorkingDirectoryInvalidPath {
                path,
                invalid_chars,
            } => {
                serde_json::json!({
                    "path": path,
                    "error": "invalid_characters",
                    "invalidCharacters": invalid_chars
                })
            }

            Self::WorkingDirectoryNetworkPath { path, suggestion } => {
                serde_json::json!({
                    "path": path,
                    "error": "network_path_not_supported",
                    "suggestion": suggestion
                })
            }

            Self::McpServerExecutableNotFound {
                server_name,
                command,
                suggestion,
            } => {
                serde_json::json!({
                    "serverName": server_name,
                    "command": command,
                    "error": "executable_not_found",
                    "suggestion": suggestion
                })
            }

            Self::McpServerStartupFailed {
                server_name,
                exit_code,
                stderr,
                suggestion,
            } => {
                serde_json::json!({
                    "serverName": server_name,
                    "exitCode": exit_code,
                    "stderr": stderr,
                    "suggestion": suggestion
                })
            }

            Self::McpServerConnectionFailed {
                server_name,
                error,
                transport_type,
            } => {
                serde_json::json!({
                    "serverName": server_name,
                    "error": error,
                    "transportType": transport_type
                })
            }

            Self::McpServerAuthenticationFailed {
                server_name,
                transport_type,
                details,
            } => {
                serde_json::json!({
                    "serverName": server_name,
                    "transportType": transport_type,
                    "error": "authentication_failed",
                    "details": details
                })
            }

            Self::McpServerTimeout {
                server_name,
                timeout_ms,
                transport_type,
            } => {
                serde_json::json!({
                    "serverName": server_name,
                    "timeoutMs": timeout_ms,
                    "transportType": transport_type,
                    "error": "connection_timeout"
                })
            }

            Self::McpServerProtocolNegotiationFailed {
                server_name,
                expected_version,
                actual_version,
            } => {
                serde_json::json!({
                    "serverName": server_name,
                    "expectedVersion": expected_version,
                    "actualVersion": actual_version,
                    "error": "protocol_negotiation_failed"
                })
            }

            Self::SessionNotFound {
                session_id,
                available_sessions,
            } => {
                serde_json::json!({
                    "sessionId": session_id.0,
                    "error": "session_not_found",
                    "availableSessions": available_sessions
                })
            }

            Self::SessionExpired {
                session_id,
                expired_at,
                max_age_seconds,
            } => {
                serde_json::json!({
                    "sessionId": session_id.0,
                    "expiredAt": expired_at,
                    "maxAgeSeconds": max_age_seconds,
                    "error": "session_expired"
                })
            }

            Self::SessionCorrupted {
                session_id,
                corruption_details,
            } => {
                serde_json::json!({
                    "sessionId": session_id.0,
                    "error": "session_corrupted",
                    "details": corruption_details
                })
            }

            Self::SessionStorageFailure {
                session_id,
                storage_error,
                recovery_suggestion,
            } => {
                serde_json::json!({
                    "sessionId": session_id,
                    "storageError": storage_error,
                    "recoverySuggestion": recovery_suggestion,
                    "error": "storage_failure"
                })
            }

            Self::SessionHistoryReplayFailed {
                session_id,
                failed_at_message,
                total_messages,
                error_details,
            } => {
                serde_json::json!({
                    "sessionId": session_id.0,
                    "failedAtMessage": failed_at_message,
                    "totalMessages": total_messages,
                    "errorDetails": error_details,
                    "error": "history_replay_failed"
                })
            }

            Self::TransportNotSupported {
                requested_transport,
                declared_capability,
                supported_transports,
            } => {
                serde_json::json!({
                    "requestedTransport": requested_transport,
                    "declaredCapability": declared_capability,
                    "supportedTransports": supported_transports
                })
            }

            Self::LoadSessionNotSupported {
                declared_capability,
            } => {
                serde_json::json!({
                    "capability": "loadSession",
                    "declaredCapability": declared_capability,
                    "error": "capability_not_supported"
                })
            }

            Self::CapabilityFormatError {
                capability_name,
                expected_format,
                actual_value,
            } => {
                serde_json::json!({
                    "capabilityName": capability_name,
                    "expectedFormat": expected_format,
                    "actualValue": actual_value,
                    "error": "capability_format_error"
                })
            }

            Self::UnknownCapability {
                capability_name,
                known_capabilities,
            } => {
                serde_json::json!({
                    "capabilityName": capability_name,
                    "knownCapabilities": known_capabilities,
                    "error": "unknown_capability"
                })
            }

            Self::MalformedRequest {
                request_type,
                details,
                example,
            } => {
                serde_json::json!({
                    "requestType": request_type,
                    "details": details,
                    "example": example,
                    "error": "malformed_request"
                })
            }

            Self::InvalidSessionId {
                provided_id,
                expected_format,
                example,
            } => {
                serde_json::json!({
                    "providedId": provided_id,
                    "expectedFormat": expected_format,
                    "example": example,
                    "error": "invalid_session_id"
                })
            }

            Self::MissingRequiredParameter {
                request_type,
                parameter_name,
                parameter_type,
            } => {
                serde_json::json!({
                    "requestType": request_type,
                    "parameterName": parameter_name,
                    "parameterType": parameter_type,
                    "error": "missing_required_parameter"
                })
            }

            Self::InvalidParameterType(details) => {
                serde_json::json!({
                    "requestType": details.request_type,
                    "parameterName": details.parameter_name,
                    "expectedType": details.expected_type,
                    "actualType": details.actual_type,
                    "providedValue": details.provided_value,
                    "error": "invalid_parameter_type"
                })
            }

            Self::PartialSessionCleanupFailed {
                session_id,
                cleanup_errors,
                resources_not_cleaned,
            } => {
                serde_json::json!({
                    "sessionId": session_id.0,
                    "cleanupErrors": cleanup_errors,
                    "resourcesNotCleaned": resources_not_cleaned,
                    "error": "partial_cleanup_failed"
                })
            }

            Self::McpServerCleanupFailed {
                server_name,
                cleanup_error,
            } => {
                serde_json::json!({
                    "serverName": server_name,
                    "cleanupError": cleanup_error,
                    "error": "mcp_server_cleanup_failed"
                })
            }

            Self::CapabilityNotSupported {
                capability_name,
                required_for,
            } => {
                serde_json::json!({
                    "capabilityName": capability_name,
                    "requiredFor": required_for,
                    "error": "capability_not_supported"
                })
            }
        }
    }

    /// Convert SessionSetupError to agent_client_protocol::Error
    pub fn to_protocol_error(&self) -> agent_client_protocol::Error {
        let json_rpc_error = self.to_json_rpc_error();
        agent_client_protocol::Error {
            code: json_rpc_error.code,
            message: json_rpc_error.message,
            data: json_rpc_error.data,
        }
    }
}

/// Result type for session setup operations
pub type SessionSetupResult<T> = Result<T, SessionSetupError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_directory_not_absolute_error() {
        let error = SessionSetupError::WorkingDirectoryNotAbsolute {
            provided_path: PathBuf::from("./relative"),
            requirement: "absolute_path".to_string(),
            example: "/home/user/project".to_string(),
        };

        assert_eq!(error.to_json_rpc_code(), -32602);
        let data = error.to_error_data().unwrap();
        assert_eq!(data["providedPath"], "./relative");
        assert_eq!(data["requirement"], "absolute_path");
    }

    #[test]
    fn test_mcp_server_not_found_error() {
        let error = SessionSetupError::McpServerExecutableNotFound {
            server_name: "test-server".to_string(),
            command: PathBuf::from("/nonexistent/server"),
            suggestion: "Check installation".to_string(),
        };

        assert_eq!(error.to_json_rpc_code(), -32603);
        let data = error.to_error_data().unwrap();
        assert_eq!(data["serverName"], "test-server");
        assert_eq!(data["error"], "executable_not_found");
    }

    #[test]
    fn test_session_not_found_error() {
        let error = SessionSetupError::SessionNotFound {
            session_id: SessionId("sess_123".to_string().into()),
            available_sessions: vec!["sess_456".to_string()],
        };

        assert_eq!(error.to_json_rpc_code(), -32602);
        let data = error.to_error_data().unwrap();
        assert_eq!(data["sessionId"], "sess_123");
        assert_eq!(data["error"], "session_not_found");
    }

    #[test]
    fn test_transport_not_supported_error() {
        let error = SessionSetupError::TransportNotSupported {
            requested_transport: "http".to_string(),
            declared_capability: false,
            supported_transports: vec!["stdio".to_string()],
        };

        assert_eq!(error.to_json_rpc_code(), -32602);
        let data = error.to_error_data().unwrap();
        assert_eq!(data["requestedTransport"], "http");
        assert_eq!(data["declaredCapability"], false);
    }

    #[test]
    fn test_protocol_error_conversion() {
        let error = SessionSetupError::InvalidSessionId {
            provided_id: "invalid".to_string(),
            expected_format: "ULID format".to_string(),
            example: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        };

        let protocol_error = error.to_protocol_error();
        assert_eq!(protocol_error.code, -32602);
        assert!(protocol_error.message.contains("Invalid session ID format"));
        assert!(protocol_error.data.is_some());
    }
}
