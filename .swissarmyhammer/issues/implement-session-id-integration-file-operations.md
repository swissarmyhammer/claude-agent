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