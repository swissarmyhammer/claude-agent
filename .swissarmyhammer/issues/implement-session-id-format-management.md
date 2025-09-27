# Implement Proper Session ID Format and Management

## Problem
Our session ID generation and management may not follow ACP specification requirements for format, uniqueness, and persistence. We need to implement proper session ID handling that supports both session creation and loading.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/session-setup:

**Session ID Requirements:**
- Unique identifier for conversation context
- Used for `session/prompt`, `session/cancel`, `session/load` operations  
- Must persist across session loads
- Should follow a consistent, recognizable format

**Example Format:**
```json
{
  "result": {
    "sessionId": "sess_abc123def456"
  }
}
```

## Current Issues
- Session ID format may not be consistent or recognizable
- Uniqueness guarantees unclear
- Session ID persistence for loading may not be implemented
- No validation of session ID format in requests
- Missing session lifecycle management

## Implementation Tasks

### Session ID Generation
- [ ] Implement consistent session ID format (e.g., `sess_` prefix + unique suffix)
- [ ] Use cryptographically secure random generation for uniqueness
- [ ] Ensure session IDs are URL-safe and filesystem-safe  
- [ ] Add length requirements for session IDs (sufficient entropy)
- [ ] Consider using ULID or similar sortable identifier format

### Session ID Validation
- [ ] Validate session ID format in all session-related requests
- [ ] Reject malformed session IDs with proper error responses
- [ ] Add session ID parsing and validation utilities
- [ ] Ensure consistent validation across all session operations

### Session Registry Management
- [ ] Implement session registry to track active sessions
- [ ] Add session lookup by ID with proper error handling
- [ ] Implement session creation and registration
- [ ] Add session cleanup and expiration policies
- [ ] Handle session state transitions (created, active, expired)

### Session Persistence
- [ ] Store session metadata with persistent session ID
- [ ] Implement session ID to storage mapping
- [ ] Ensure session IDs persist across agent restarts
- [ ] Add session storage backend abstraction
- [ ] Handle storage failures gracefully

### Session Lifecycle Management
- [ ] Track session creation timestamps
- [ ] Implement session expiration policies
- [ ] Add session cleanup for expired/unused sessions
- [ ] Handle session resource cleanup (MCP connections, etc.)
- [ ] Add session usage tracking for management

## Session ID Format Specification
Implement consistent format:
```rust
// Session ID format: sess_<timestamp><random>
// Example: sess_1703123456_abc123def456
// - sess_: Fixed prefix for recognition
// - timestamp: Creation time for sorting/expiration
// - random: Cryptographically secure random suffix
```

## Error Handling
Proper error responses for session ID issues:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid session ID format: must start with 'sess_'",
    "data": {
      "providedSessionId": "invalid-123",
      "expectedFormat": "sess_<timestamp>_<random>",
      "example": "sess_1703123456_abc123def456"
    }
  }
}
```

```json
{
  "error": {
    "code": -32602,
    "message": "Session not found: sessionId does not exist",
    "data": {
      "sessionId": "sess_nonexistent123",
      "availableSessions": ["sess_abc123", "sess_def456"]
    }
  }
}
```

## Implementation Notes
Add session ID management comments:
```rust
// ACP session ID requirements:
// 1. Unique identifier for conversation context
// 2. Must persist across session loads  
// 3. Used in session/prompt, session/cancel, session/load
// 4. Should follow consistent, recognizable format
// 5. Must be URL-safe and filesystem-safe
//
// Format: sess_<timestamp>_<random> for uniqueness and debugging
```

## Session Registry Implementation
```rust
pub struct SessionRegistry {
    sessions: HashMap<SessionId, SessionInfo>,
    creation_times: HashMap<SessionId, SystemTime>,
    expiration_policy: ExpirationPolicy,
}

impl SessionRegistry {
    pub fn create_session(&mut self, config: SessionConfig) -> SessionId;
    pub fn get_session(&self, id: &SessionId) -> Option<&SessionInfo>;
    pub fn remove_expired(&mut self) -> Vec<SessionId>;
    pub fn validate_session_id(id: &str) -> Result<SessionId, ValidationError>;
}
```

## Testing Requirements  
- [ ] Test session ID generation uniqueness across multiple calls
- [ ] Test session ID format validation with various invalid inputs
- [ ] Test session registry operations (create, lookup, cleanup)
- [ ] Test session persistence across storage operations
- [ ] Test session expiration and cleanup policies
- [ ] Test concurrent session creation and access
- [ ] Test session ID collision handling (unlikely but possible)
- [ ] Test storage backend failures and recovery

## Integration Points
- [ ] Update all session-related handlers to use new session ID system
- [ ] Integrate with session loading for ID validation
- [ ] Connect to session persistence storage
- [ ] Add session ID validation middleware for requests
- [ ] Update session cleanup in server shutdown

## Acceptance Criteria
- Consistent session ID format following ACP requirements
- Cryptographically unique session ID generation  
- Proper session ID validation in all requests
- Session registry for tracking active sessions
- Session persistence supporting load operations
- Session expiration and cleanup policies
- Comprehensive error handling with proper ACP error codes
- Complete test coverage for all session ID scenarios
- Integration with existing session management code