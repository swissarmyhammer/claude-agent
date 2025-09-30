# Implement Session ID Integration for File Operations

## Problem
Our file system operations may not properly validate and integrate session IDs as required by the ACP specification. All file system methods require valid session IDs for correlation with active sessions, access control, and operation tracking.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/file-system:

**Session ID Requirements:**
- All file system methods require `sessionId` parameter
- Session ID must correlate with active session
- File operations must be tracked per session
- Session context affects file access permissions and boundaries

**Method Examples:**
```json
{
  "method": "fs/read_text_file",
  "params": {
    "sessionId": "sess_abc123def456",  // Required
    "path": "/home/user/project/src/main.py"
  }
}
```

```json
{
  "method": "fs/write_text_file", 
  "params": {
    "sessionId": "sess_abc123def456",  // Required
    "path": "/home/user/project/config.json",
    "content": "{\"debug\": true}"
  }
}
```

## Current Issues
- Session ID validation for file operations unclear
- No correlation between file operations and active sessions
- Missing session-based access control and permissions
- File operation tracking per session not implemented

## Implementation Tasks

### Session ID Validation
- [ ] Validate session ID format and structure
- [ ] Check session ID against active session registry
- [ ] Reject file operations for invalid or expired sessions
- [ ] Add proper error handling for session validation failures

### Session Context Integration
- [ ] Retrieve session context for file operations
- [ ] Apply session-based working directory boundaries
- [ ] Use session configuration for file access permissions
- [ ] Track file operations within session lifecycle

### File Operation Tracking
- [ ] Track file read operations per session
- [ ] Track file write operations per session
- [ ] Maintain session-based file operation history
- [ ] Add file operation metrics and monitoring per session

### Access Control Integration
- [ ] Apply session-based file access restrictions
- [ ] Check file paths against session working directory
- [ ] Validate file operations against session permissions
- [ ] Implement session-specific file operation policies

## Session Integration Implementation
```rust
pub struct FileSystemSessionManager {
    session_registry: Arc<SessionRegistry>,
    operation_tracker: FileOperationTracker,
}

impl FileSystemSessionManager {
    pub async fn validate_session_for_file_operation(
        &self,
        session_id: &str,
        operation_type: FileOperationType,
        path: &str,
    ) -> Result<SessionContext, SessionValidationError> {
        // Validate session exists and is active
        let session = self.session_registry.get_active_session(session_id)
            .ok_or(SessionValidationError::SessionNotFound(session_id.to_string()))?;
        
        // Check if session allows file operations
        if !session.capabilities.file_system_access {
            return Err(SessionValidationError::FileSystemNotAllowed);
        }
        
        // Validate path is within session boundaries
        self.validate_path_within_session_boundaries(&session, path)?;
        
        // Check operation-specific permissions
        self.validate_operation_permissions(&session, operation_type)?;
        
        // Track operation for session
        self.operation_tracker.track_operation(session_id, operation_type, path).await;
        
        Ok(session)
    }
    
    fn validate_path_within_session_boundaries(
        &self,
        session: &SessionContext,
        path: &str,
    ) -> Result<(), SessionValidationError> {
        let session_root = &session.working_directory;
        let normalized_path = PathBuf::from(path);
        
        if !normalized_path.starts_with(session_root) {
            return Err(SessionValidationError::PathOutsideSessionBoundary {
                path: path.to_string(),
                session_root: session_root.to_string_lossy().to_string(),
            });
        }
        
        Ok(())
    }
}

#[derive(Debug)]
pub enum FileOperationType {
    Read,
    Write,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionValidationError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    
    #[error("File system access not allowed for this session")]
    FileSystemNotAllowed,
    
    #[error("Path outside session boundary: {path} not within {session_root}")]
    PathOutsideSessionBoundary { path: String, session_root: String },
    
    #[error("Operation not permitted: {operation:?}")]
    OperationNotPermitted { operation: FileOperationType },
}
```

## Implementation Notes
Add session integration comments:
```rust
// ACP requires session ID validation for all file operations:
// 1. Validate sessionId parameter in all fs/ method requests
// 2. Check session exists and is active in session registry
// 3. Apply session-based access control and path boundaries
// 4. Track file operations within session context
// 5. Use session working directory for path validation
//
// Session integration ensures proper access control and operation tracking.
```

### File Operation Handler Updates
```rust
pub async fn handle_read_text_file(
    params: ReadTextFileParams,
    session_manager: &FileSystemSessionManager,
) -> Result<ReadTextFileResponse, FileSystemError> {
    // Validate session and get context
    let session_context = session_manager.validate_session_for_file_operation(
        &params.session_id,
        FileOperationType::Read,
        &params.path,
    ).await?;
    
    // Perform file read with session context
    let content = read_file_with_session_context(&params.path, &session_context).await?;
    
    Ok(ReadTextFileResponse { content })
}
```

### Session-Based Working Directory
- [ ] Use session working directory as base for relative path resolution
- [ ] Validate file paths are within session working directory boundaries
- [ ] Support session-specific file access policies
- [ ] Add session working directory configuration and management

### File Operation Tracking
```rust
pub struct FileOperationTracker {
    operations: Arc<Mutex<HashMap<String, Vec<FileOperation>>>>,
}

#[derive(Debug, Clone)]
pub struct FileOperation {
    pub operation_type: FileOperationType,
    pub path: String,
    pub timestamp: SystemTime,
    pub result: FileOperationResult,
}

impl FileOperationTracker {
    pub async fn track_operation(
        &self,
        session_id: &str,
        operation_type: FileOperationType,
        path: &str,
    ) {
        let operation = FileOperation {
            operation_type,
            path: path.to_string(),
            timestamp: SystemTime::now(),
            result: FileOperationResult::Pending,
        };
        
        let mut operations = self.operations.lock().await;
        operations.entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(operation);
    }
}
```

### Error Handling and Responses
For invalid session ID:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid session: session not found or expired",
    "data": {
      "sessionId": "sess_invalid123",
      "error": "session_not_found",
      "suggestion": "Ensure session is active and valid"
    }
  }
}
```

For path outside session boundary:
```json
{
  "error": {
    "code": -32602,
    "message": "Path outside session boundary",
    "data": {
      "path": "/etc/passwd",
      "sessionId": "sess_abc123def456",
      "sessionWorkingDirectory": "/home/user/project",
      "error": "path_outside_boundary"
    }
  }
}
```

## Testing Requirements
- [ ] Test session ID validation for valid and invalid sessions
- [ ] Test file operations rejected for expired/non-existent sessions
- [ ] Test path boundary validation within session working directory
- [ ] Test file operation tracking per session
- [ ] Test session-based access control and permissions
- [ ] Test concurrent file operations across multiple sessions
- [ ] Test session cleanup and file operation history management

## Integration Points
- [ ] Connect to session registry and management system
- [ ] Integrate with file system method handlers
- [ ] Connect to access control and permission systems
- [ ] Integrate with operation tracking and monitoring

## Session Lifecycle Integration
- [ ] Clean up file operation tracking when sessions end
- [ ] Handle session expiration during file operations
- [ ] Support session migration and file operation continuity
- [ ] Add session-specific file operation limits and quotas

## Acceptance Criteria
- Session ID validation for all file system operations
- Integration with active session registry and lifecycle
- Session-based path boundary validation and access control
- File operation tracking and history per session
- Proper error handling for session validation failures
- Integration with existing file system method implementations
- Performance optimization for session validation overhead
- Comprehensive test coverage for all session integration scenarios
- Clear error messages for session-related failures
- Documentation of session integration requirements and behavior

## Proposed Solution

Based on my analysis of the existing codebase, I've identified that:

1. **Session Management Already Exists**: The codebase has a robust session management system with `SessionManager` and `Session` types that track working directories, session IDs, and session state.

2. **File Operations Are Implemented**: File operations (`fs_read`, `fs_write`, `fs_list`) are handled in `tools.rs` via the `ToolCallHandler`.

3. **Path Validation Is Present**: The `PathValidator` provides comprehensive path security validation including absolute path checks, traversal prevention, and boundary enforcement.

4. **Gap Identified**: File operations currently don't correlate with session context - they don't validate paths against session working directories or track operations per session.

### Implementation Strategy

#### Phase 1: Add Session Context to File Operations

1. **Extend ToolCallHandler** to store session working directory context
   - Pass session working directory when handling file operations
   - Use session context for path resolution and validation

2. **Integrate PathValidator with Session Boundaries**
   - Configure PathValidator with session's working directory as allowed root
   - Validate all file paths are within session boundaries
   - Reject operations outside session working directory

#### Phase 2: Session-Scoped File Operation Tracking

3. **Create FileOperationTracker**
   - Track file read/write operations per session
   - Store operation history with timestamps and results
   - Integrate with existing `ToolCallReport` system

4. **Add Session Validation** 
   - Verify session ID exists and is active before file operations
   - Check session capabilities for file system access
   - Return appropriate ACP error codes for invalid sessions

#### Phase 3: Enhanced Error Handling

5. **Add Session-Specific Error Types**
   - Session not found or expired errors
   - Path outside session boundary errors  
   - Session lacks file system capability errors

6. **Update Error Responses**
   - Include session context in error messages
   - Provide clear actionable guidance

### Detailed Implementation Steps

#### Step 1: Add Session Working Directory to File Operations

Modify `handle_fs_read` and `handle_fs_write` to accept session context:

```rust
async fn handle_fs_read(
    &self, 
    session_id: &agent_client_protocol::SessionId,
    session_cwd: &Path,
    request: &InternalToolRequest
) -> crate::Result<String>
```

#### Step 2: Configure PathValidator with Session Boundaries

```rust
fn validate_file_path_with_session(
    &self,
    path_str: &str,
    session_cwd: &Path
) -> crate::Result<PathBuf> {
    let validator = PathValidator::with_allowed_roots(vec![session_cwd.to_path_buf()]);
    let validated_path = validator.validate_absolute_path(path_str)
        .map_err(|e| match e {
            PathValidationError::OutsideBoundaries(p) => {
                crate::AgentError::Session(format!(
                    "Path outside session boundary: {} not within {}",
                    p, session_cwd.display()
                ))
            },
            _ => crate::AgentError::ToolExecution(format!("Path validation failed: {}", e))
        })?;
    Ok(validated_path)
}
```

#### Step 3: Update execute_tool_request to Pass Session Context

Modify `execute_tool_request` to retrieve session and pass working directory:

```rust
async fn execute_tool_request(
    &self,
    session_id: &agent_client_protocol::SessionId,
    session_manager: &SessionManager,
    request: &InternalToolRequest
) -> crate::Result<String> {
    // Get session context
    let session = session_manager.get_session(session_id)?
        .ok_or_else(|| crate::AgentError::Session(format!(
            "Session not found: {}", session_id
        )))?;
    
    let session_cwd = &session.cwd;
    
    match request.name.as_str() {
        "fs_read" => self.handle_fs_read(session_id, session_cwd, request).await,
        "fs_write" => self.handle_fs_write(session_id, session_cwd, request).await,
        // ... other tools
    }
}
```

#### Step 4: Add File Operation Tracking

Create tracking structure:

```rust
#[derive(Debug, Clone)]
pub struct FileOperation {
    pub operation_type: FileOperationType,
    pub path: PathBuf,
    pub timestamp: SystemTime,
    pub result: FileOperationResult,
}

#[derive(Debug, Clone)]
pub enum FileOperationType {
    Read,
    Write,
    List,
}

#[derive(Debug, Clone)]
pub enum FileOperationResult {
    Success,
    Failed(String),
}
```

Add to ToolCallHandler:

```rust
file_operations: Arc<RwLock<HashMap<String, Vec<FileOperation>>>>,
```

Track operations:

```rust
async fn track_file_operation(
    &self,
    session_id: &str,
    operation_type: FileOperationType,
    path: &Path,
    result: FileOperationResult,
) {
    let operation = FileOperation {
        operation_type,
        path: path.to_path_buf(),
        timestamp: SystemTime::now(),
        result,
    };
    
    let mut ops = self.file_operations.write().await;
    ops.entry(session_id.to_string())
        .or_insert_with(Vec::new)
        .push(operation);
}
```

### Testing Strategy

1. **Test session validation** - ensure operations fail for invalid session IDs
2. **Test path boundaries** - verify paths outside session CWD are rejected
3. **Test operation tracking** - confirm all file operations are recorded per session
4. **Test error handling** - validate proper error codes and messages
5. **Test concurrent sessions** - ensure isolation between multiple sessions

### Implementation Notes

- Use existing `SessionManager` infrastructure rather than creating parallel systems
- Leverage `PathValidator` for security - don't duplicate validation logic
- Integrate with `ToolCallReport` tracking for consistency
- Maintain backward compatibility with existing tool interfaces where possible
- Follow TDD approach: write failing tests first, then implement features

## Progress Report

### Current Status: Implementation in Progress

I've successfully completed the TDD setup phase and begun implementation:

#### Completed:
1. ✅ Analyzed existing session management and file operation infrastructure
2. ✅ Designed comprehensive solution integrating sessions with file operations
3. ✅ Added file operation tracking types and infrastructure to `ToolCallHandler`
4. ✅ Created three failing tests that validate our requirements:
   - `test_file_operation_requires_valid_session` - passes (no validation yet)
   - `test_file_operation_respects_session_boundary` - fails as expected
   - `test_file_operation_tracking_per_session` - fails as expected (0 tracked vs 2 expected)

#### Implementation Added:

**New Types** (lib/src/tools.rs):
```rust
pub enum FileOperationType { Read, Write, List }
pub enum FileOperationResult { Success, Failed(String) }
pub struct FileOperation {
    pub operation_type: FileOperationType,
    pub path: PathBuf,
    pub timestamp: SystemTime,
    pub result: FileOperationResult,
}
```

**ToolCallHandler Extensions**:
- Added `file_operations: Arc<RwLock<HashMap<String, Vec<FileOperation>>>>` field
- Implemented `track_file_operation()` method for recording operations per session
- Implemented `get_file_operations()` method for retrieving session-specific history

#### Next Steps:

1. **Integrate tracking into file operations** - Modify `handle_fs_read`, `handle_fs_write`, and `handle_fs_list` to call `track_file_operation()` after each successful/failed operation

2. **Add session context to file operations** - Pass session manager and validate session exists before executing file operations

3. **Implement session boundary validation** - Configure `PathValidator` with session working directory and reject paths outside boundaries

4. **Wire up session manager** - Modify `execute_tool_request` to accept session manager reference and retrieve session context

### Test Results:
```
Summary: 3 tests run: 1 passed, 2 failed, 430 skipped
FAIL test_file_operation_respects_session_boundary - Files outside session boundary not rejected yet
FAIL test_file_operation_tracking_per_session - Operations not being tracked (0 vs 2 expected)
```

These failures are expected at this stage - they confirm our tests are properly structured and ready to drive implementation.