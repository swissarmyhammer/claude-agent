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

## Proposed Solution

After analyzing the current codebase, here's my implementation plan:

### Current State Analysis

1. **SessionId Type**: Currently defined as `pub type SessionId = Ulid` in `lib/src/session.rs:11`
2. **Format**: Raw ULID without prefix (e.g., "01ARZ3NDEKTSV4RRFFQ69G5FAV")
3. **Validation**: Basic ULID validation exists in `session_validation.rs:validate_session_id()`
4. **Usage**: Session IDs are used throughout:
   - Session creation in `SessionManager::create_session()`
   - Session/prompt, session/cancel, session/load handlers
   - Protocol conversions with `agent_client_protocol::SessionId`

### Issues Identified

1. No recognizable prefix for human identification (ACP recommends "sess_")
2. Session ID is just a type alias, not a proper newtype with enforced validation
3. No format documentation in responses for client guidance
4. Tests use arbitrary strings like "test_session_123" without validation

### Implementation Strategy

#### Phase 1: Create SessionId Newtype with Format Management
- Replace `pub type SessionId = Ulid` with a proper newtype struct
- Implement format: `sess_<ulid>` (e.g., "sess_01ARZ3NDEKTSV4RRFFQ69G5FAV")
- Add validation, serialization, and display traits
- Support both creation and parsing of formatted IDs

#### Phase 2: Update Session Creation
- Modify `SessionManager::create_session()` to generate formatted IDs
- Ensure backward compatibility during transition
- Update all internal ULID generation to use new format

#### Phase 3: Update Validation Layer
- Enhance `validate_session_id()` to check for "sess_" prefix
- Provide detailed error messages with format examples
- Add validation in all request handlers

#### Phase 4: Update Error Messages
- Update all error responses to include format expectations
- Add examples of valid session IDs in error data
- Update session not found errors with format guidance

#### Phase 5: Comprehensive Testing
- Test session ID generation uniqueness
- Test format validation with various invalid inputs
- Test serialization/deserialization
- Test backward compatibility if needed
- Update existing tests to use proper format

### Detailed Design

```rust
/// Session identifier with ACP-compliant format
///
/// Format: sess_<ULID>
/// Example: sess_01ARZ3NDEKTSV4RRFFQ69G5FAV
///
/// The prefix "sess_" provides human-recognizable format while
/// ULID provides uniqueness, sortability, and timestamp information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SessionId(Ulid);

impl SessionId {
    const PREFIX: &'static str = "sess_";
    
    /// Create a new session ID with proper format
    pub fn new() -> Self {
        Self(Ulid::new())
    }
    
    /// Parse a session ID from string
    pub fn parse(s: &str) -> Result<Self, SessionIdError> {
        // Validate format and parse
    }
    
    /// Get the underlying ULID
    pub fn as_ulid(&self) -> Ulid {
        self.0
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}
```

### Migration Considerations

1. **No Storage Migration Needed**: Internal storage still uses ULID
2. **Format Only in Protocol**: Add/remove prefix at protocol boundaries
3. **Validation at Entry Points**: Check format when receiving from clients
4. **Clear Error Messages**: Guide clients to proper format

### Testing Strategy

1. Unit tests for SessionId creation and parsing
2. Integration tests for session/setup flow
3. Error case tests for invalid formats
4. Round-trip serialization tests
5. Backward compatibility tests if needed


## Implementation Complete

### Summary

Successfully implemented proper session ID format management with the `sess_` prefix according to ACP requirements. All tests are passing (430/430).

### Changes Made

#### 1. SessionId Newtype Implementation (lib/src/session.rs)
- Replaced `pub type SessionId = Ulid` with a proper newtype struct
- Implemented format: `sess_<ULID>` (e.g., `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV`)
- Added comprehensive validation, parsing, serialization, and display traits
- Created `SessionIdError` enum for detailed error handling
- Implemented: `new()`, `parse()`, `as_ulid()`, `ulid_string()`
- Added trait implementations: `Display`, `FromStr`, `From<Ulid>`, `Serialize`, `Deserialize`

#### 2. Validation Updates (lib/src/session_validation.rs)
- Updated `validate_session_id()` to check for `sess_` prefix format
- Enhanced error messages with format examples
- Updated tests to use new format

#### 3. Session Management Updates
- Updated `SessionManager::create_session()` to generate formatted IDs
- Updated all session operations to use new SessionId type
- Updated `parse_session_id()` in agent.rs to parse new format

#### 4. Handler Updates (lib/src/agent.rs, lib/src/terminal_manager.rs)
- Updated all session ID parsing throughout the codebase
- Fixed `handle_streaming_prompt()` and `handle_non_streaming_prompt()` signatures
- Updated `update_session_available_commands()` to parse new format
- Updated terminal validation to use new SessionId parsing

#### 5. Comprehensive Testing
- Added 12 new tests for SessionId functionality in session.rs:
  - Format validation (prefix, ULID, empty, missing parts)
  - Serialization/deserialization
  - Display and FromStr traits
  - URL-safe character validation
- Updated all existing tests to use `sess_` prefix format
- Fixed tests in session.rs, session_validation.rs, session_loading.rs, request_validation.rs, tools.rs, agent.rs

### Test Results
```
Summary [14.179s] 430 tests run: 430 passed (1 leaky), 0 skipped
```

### Format Examples

**Valid Session IDs:**
- `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV`
- `sess_01K6D6J0SVV1GA4BBP6R698XP9`

**Invalid Session IDs:**
- `01ARZ3NDEKTSV4RRFFQ69G5FAV` (missing prefix)
- `session_123` (wrong format)
- `sess_` (missing ULID)
- `sess_invalid-ulid` (invalid ULID)

### Error Messages

Error messages now include format guidance:
- Expected format: `sess_<ULID> format (sess_ prefix + 26-character ULID)`
- Example: `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV`

### Benefits

1. **Human Recognition**: The `sess_` prefix makes session IDs immediately recognizable
2. **Consistency**: Enforced format across all session operations
3. **Validation**: Comprehensive validation at all entry points
4. **Type Safety**: Proper newtype prevents accidental ULID/SessionId mixing
5. **ACP Compliance**: Meets all ACP requirements for session ID format
6. **Backward Compatibility**: Internal storage still uses ULID for efficiency
7. **URL/Filesystem Safe**: Only uses alphanumeric characters and underscores

### Next Steps for Issue Completion

The session ID format implementation is complete. The following tasks from the original issue remain:

- [ ] Session Registry Management (if needed beyond current SessionManager)
- [ ] Session Persistence (storage backend for session/load)
- [ ] Session Lifecycle Management (expiration policies are basic)
- [ ] Additional error handling improvements

However, the core session ID format requirement has been fully implemented and tested.

## Code Review Fixes Applied

Completed all action items from code review:

### 1. Fixed Lint Error
- **lib/src/agent.rs:5801**: Removed unused `SessionId` import from test
  - Changed `use crate::session::{Session, SessionId}` to `use crate::session::Session`
  - SessionId was imported but fully qualified usage made it redundant

### 2. Fixed Unused Variable
- **lib/src/session_validation.rs:150**: Changed `Err(_e)` to `Err(_)`
  - Error variable was captured but never used in error message
  - Simplified to anonymous pattern match

### 3. Updated Temporal Comments
- **lib/src/agent.rs:1054**: Changed "Parse session ID with new sess_ prefix format" to "Parse session ID from ACP format (sess_<ULID>) to internal SessionId type"
- **lib/src/agent.rs:1827**: Changed "Parse SessionId with new sess_ prefix format" to "Parse SessionId from ACP format (sess_<ULID>)"
- Removed temporal reference "new" to make comments evergreen

### 4. Enhanced Documentation
- **SessionIdError variants**: Added comprehensive documentation to each error variant
  - `Empty`: Explains when empty string is provided and what to do
  - `MissingPrefix`: Explains ACP requirement, includes valid/invalid examples
  - `MissingUlid`: Explains when prefix exists but ULID is missing, includes examples
  - `InvalidUlid`: Explains ULID format requirements (26 chars, Crockford Base32), includes examples
  
### 5. Documented ulid_string() Method
- **lib/src/session.rs:87-98**: Added detailed documentation for `ulid_string()` method
  - Explains use case: backward compatibility with internal storage systems
  - Notes the difference between ACP protocol format (with prefix) and internal format (raw ULID)
  - Includes code example showing input/output relationship

### Verification
All changes verified with:
- ✅ `cargo build` - Compiles successfully in 2.37s
- ✅ `cargo clippy --all-targets --all-features -- -D warnings` - No lint errors or warnings
- ✅ `cargo nextest run` - All 430 tests passing (1 leaky)

### Files Modified
- lib/src/agent.rs (unused import, temporal comments)
- lib/src/session_validation.rs (unused error variable)
- lib/src/session.rs (SessionIdError documentation, ulid_string() documentation)