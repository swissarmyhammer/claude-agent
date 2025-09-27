# Implement ACP Session Setup Error Handling

## Problem
Our session setup implementation lacks comprehensive error handling for the various failure scenarios specified in the ACP specification. We need proper error responses, error codes, and graceful handling for all session-related failures.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/session-setup:

**Error Scenarios to Handle:**
- Invalid working directory paths
- MCP server connection failures
- Session loading failures  
- Transport capability mismatches
- Session not found errors
- Permission and access issues

## Current Issues
- Missing comprehensive error handling for session setup failures
- No standardized error response format for session operations
- MCP server connection failures may not be properly handled
- Working directory validation errors unclear
- Session loading error responses incomplete

## Implementation Tasks

### Working Directory Errors
- [ ] Handle non-existent directory paths
- [ ] Handle permission denied errors
- [ ] Handle relative path validation failures
- [ ] Handle path format errors (invalid characters, etc.)
- [ ] Handle network path and mount issues

### MCP Server Connection Errors
- [ ] Handle MCP server executable not found
- [ ] Handle MCP server startup failures
- [ ] Handle HTTP/SSE connection timeouts
- [ ] Handle authentication failures for HTTP/SSE
- [ ] Handle network connectivity issues
- [ ] Handle MCP protocol negotiation failures

### Session Loading Errors
- [ ] Handle session not found errors
- [ ] Handle expired session errors
- [ ] Handle corrupted session data
- [ ] Handle storage backend failures
- [ ] Handle session history replay failures

### Capability Validation Errors
- [ ] Handle transport type not supported errors
- [ ] Handle loadSession capability not supported
- [ ] Handle capability format validation errors
- [ ] Handle unknown capability errors

### Request Validation Errors
- [ ] Handle malformed session/new requests
- [ ] Handle malformed session/load requests
- [ ] Handle invalid session ID format errors
- [ ] Handle missing required parameters
- [ ] Handle invalid parameter types

## Error Response Examples

### Working Directory Errors
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid working directory: path must be absolute",
    "data": {
      "providedPath": "./relative/path", 
      "requirement": "absolute_path",
      "example": "/home/user/project"
    }
  }
}
```

```json
{
  "error": {
    "code": -32603,
    "message": "Working directory access denied: insufficient permissions",
    "data": {
      "path": "/root/restricted",
      "error": "permission_denied",
      "requiredPermissions": ["read", "execute"]
    }
  }
}
```

### MCP Server Connection Errors
```json
{
  "error": {
    "code": -32603,
    "message": "MCP server connection failed: executable not found",
    "data": {
      "serverName": "filesystem",
      "command": "/nonexistent/mcp-server",
      "error": "executable_not_found",
      "suggestion": "Check server installation and path"
    }
  }
}
```

```json
{
  "error": {
    "code": -32603,
    "message": "MCP server startup failed: process exited with code 1",
    "data": {
      "serverName": "api-server",
      "exitCode": 1,
      "stderr": "Configuration error: missing API key",
      "suggestion": "Check server configuration and environment variables"
    }
  }
}
```

### Session Loading Errors
```json
{
  "error": {
    "code": -32602,
    "message": "Session not found: sessionId does not exist or has expired",
    "data": {
      "sessionId": "sess_invalid123",
      "error": "session_not_found",
      "availableSessions": ["sess_abc123", "sess_def456"]
    }
  }
}
```

### Transport Capability Errors
```json
{
  "error": {
    "code": -32602,
    "message": "Transport not supported: agent does not support HTTP transport",
    "data": {
      "requestedTransport": "http",
      "declaredCapability": false,
      "supportedTransports": ["stdio"]
    }
  }
}
```

## Error Classification System
Implement consistent error classification:
```rust
#[derive(Debug)]
pub enum SessionSetupError {
    // Working directory errors
    WorkingDirectoryNotFound(PathBuf),
    WorkingDirectoryPermissionDenied(PathBuf),
    WorkingDirectoryNotAbsolute(PathBuf),
    
    // MCP server errors  
    McpServerNotFound(String, PathBuf),
    McpServerStartupFailed(String, i32, String),
    McpServerConnectionFailed(String, String),
    
    // Session errors
    SessionNotFound(SessionId),
    SessionExpired(SessionId),
    SessionLoadFailed(SessionId, String),
    
    // Capability errors
    TransportNotSupported(String),
    LoadSessionNotSupported,
    
    // Validation errors
    InvalidSessionId(String),
    MalformedRequest(String),
}
```

## Implementation Notes
Add comprehensive error handling comments:
```rust
// ACP requires comprehensive error handling for session setup:
// 1. Clear, actionable error messages for clients
// 2. Appropriate JSON-RPC error codes  
// 3. Structured error data for programmatic handling
// 4. Graceful degradation where possible
// 5. Proper cleanup of partial session state on failures
```

## Error Recovery and Cleanup
- [ ] Implement partial session cleanup on failures
- [ ] Handle MCP server connection cleanup on errors
- [ ] Clean up file system resources on working directory failures
- [ ] Implement graceful degradation for non-critical MCP server failures
- [ ] Add retry logic for transient failures where appropriate

## Logging and Monitoring
- [ ] Add structured logging for all error scenarios
- [ ] Include error correlation IDs for debugging
- [ ] Add metrics for error frequency and types
- [ ] Log sufficient detail for troubleshooting without exposing sensitive data

## Testing Requirements
- [ ] Test all working directory error scenarios
- [ ] Test MCP server connection failure handling
- [ ] Test session loading error cases
- [ ] Test capability validation error responses
- [ ] Test malformed request handling
- [ ] Test partial session cleanup on failures
- [ ] Test error response format compliance
- [ ] Test error logging and correlation

## Integration Points
- [ ] Integrate error handling with existing session management
- [ ] Connect to MCP server connection management
- [ ] Add error handling to session persistence layer
- [ ] Update all session-related handlers with proper error responses

## Acceptance Criteria
- Comprehensive error handling for all session setup failure modes
- Proper JSON-RPC error codes following ACP standards
- Clear, actionable error messages for client debugging
- Structured error data for programmatic handling
- Graceful cleanup of partial session state on failures  
- Consistent error response format across all session operations
- Complete test coverage for all error scenarios
- Proper logging and monitoring for error tracking
- Integration with existing session management without breaking changes