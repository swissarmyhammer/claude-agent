# Implement Complete fs/write_text_file Method

## Problem
Our file writing implementation may not fully comply with the ACP specification requirements. We need complete support for the `fs/write_text_file` method including proper file creation behavior, atomic operations, and error handling.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/file-system:

**Method Signature:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "fs/write_text_file",
  "params": {
    "sessionId": "sess_abc123def456",
    "path": "/home/user/project/config.json",
    "content": "{\n  \"debug\": true,\n  \"version\": \"1.0.0\"\n}"
  }
}
```

**Response Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": null
}
```

**Critical Requirement:**
> "The Client MUST create the file if it doesn't exist."

## Current Issues
- File writing implementation compliance with ACP specification unclear
- File creation behavior may not match "MUST create" requirement
- Missing atomic write operations for data integrity
- Directory creation for parent paths unclear

## Implementation Tasks

### Method Handler Implementation
- [ ] Implement complete `fs/write_text_file` method handler
- [ ] Add proper JSON-RPC method registration
- [ ] Support all required parameters with validation
- [ ] Return null result on successful write per specification

### Parameter Support
- [ ] Support required `sessionId` parameter with validation
- [ ] Support required `path` parameter with absolute path validation
- [ ] Support required `content` parameter with text content
- [ ] Add parameter validation and error handling

### File Creation Behavior
- [ ] Implement "MUST create file if it doesn't exist" requirement
- [ ] Create parent directories if they don't exist
- [ ] Handle file permission setting for new files
- [ ] Add proper error handling for creation failures

### Atomic Write Operations
- [ ] Implement atomic file writing to prevent data corruption
- [ ] Use temporary files with atomic rename operations
- [ ] Handle partial write failures with rollback
- [ ] Ensure file integrity during write operations

## File Writing Implementation
```rust
#[derive(Debug, Deserialize)]
pub struct WriteTextFileParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub path: String,
    pub content: String,
}

pub async fn handle_write_text_file(
    params: WriteTextFileParams
) -> Result<serde_json::Value, FileSystemError> {
    // Validate session ID
    validate_session_id(&params.session_id)?;
    
    // Validate absolute path
    validate_absolute_path(&params.path)?;
    
    // Perform atomic write operation
    write_file_atomically(&params.path, &params.content).await?;
    
    // Return null result as per ACP specification
    Ok(serde_json::Value::Null)
}

async fn write_file_atomically(
    path: &str, 
    content: &str
) -> Result<(), FileSystemError> {
    let path_buf = PathBuf::from(path);
    
    // Create parent directories if they don't exist
    if let Some(parent_dir) = path_buf.parent() {
        tokio::fs::create_dir_all(parent_dir).await
            .map_err(|e| FileSystemError::DirectoryCreationFailed(
                parent_dir.to_string_lossy().to_string(), e
            ))?;
    }
    
    // Create temporary file for atomic write
    let temp_path = format!("{}.tmp.{}", path, uuid::Uuid::new_v4());
    
    // Write content to temporary file
    tokio::fs::write(&temp_path, content).await
        .map_err(|e| FileSystemError::WriteFailed(temp_path.clone(), e))?;
    
    // Atomically rename temporary file to final path
    tokio::fs::rename(&temp_path, path).await
        .map_err(|e| {
            // Clean up temp file on failure
            let _ = std::fs::remove_file(&temp_path);
            FileSystemError::WriteFailed(path.to_string(), e)
        })?;
    
    Ok(())
}
```

## Implementation Notes
Add file writing method comments:
```rust
// ACP fs/write_text_file method implementation:
// 1. sessionId: Required - validate against active sessions
// 2. path: Required - must be absolute path
// 3. content: Required - text content to write
// 4. MUST create file if it doesn't exist per ACP specification
// 5. MUST create parent directories if needed
// 6. Response: null result on success
//
// Uses atomic write operations to ensure file integrity.
```

### Directory Creation Logic
```rust
async fn ensure_parent_directories(path: &Path) -> Result<(), FileSystemError> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| FileSystemError::DirectoryCreationFailed(
                    parent.to_string_lossy().to_string(), e
                ))?;
        }
    }
    Ok(())
}
```

### File Permission Handling
- [ ] Set appropriate permissions for newly created files
- [ ] Handle platform-specific permission requirements
- [ ] Support custom permission configuration
- [ ] Add permission error handling and recovery

### Content Validation
- [ ] Validate content is valid text (UTF-8)
- [ ] Add content size limits to prevent DoS
- [ ] Handle large content with streaming writes
- [ ] Support different text encodings where needed

### Error Handling
- [ ] Handle file permission denied errors
- [ ] Handle disk space full errors
- [ ] Handle directory creation failures
- [ ] Handle atomic write operation failures
- [ ] Provide clear error messages for debugging

## Error Response Examples
For permission denied:
```json
{
  "error": {
    "code": -32603,
    "message": "Failed to write file: permission denied",
    "data": {
      "path": "/home/user/project/config.json",
      "error": "permission_denied",
      "suggestion": "Check file and directory permissions"
    }
  }
}
```

For directory creation failure:
```json
{
  "error": {
    "code": -32603,
    "message": "Failed to create parent directory",
    "data": {
      "path": "/home/user/nonexistent/config.json",
      "parentDirectory": "/home/user/nonexistent",
      "error": "permission_denied"
    }
  }
}
```

## Testing Requirements
- [ ] Test successful file writing with new file creation
- [ ] Test file overwriting for existing files
- [ ] Test parent directory creation when directories don't exist
- [ ] Test atomic write behavior and rollback on failures
- [ ] Test error scenarios (permissions, disk space, etc.)
- [ ] Test large content writing and performance
- [ ] Test concurrent write operations
- [ ] Test file permission setting for new files

## Integration Points
- [ ] Connect to session validation system
- [ ] Integrate with client capability validation
- [ ] Connect to path validation and security systems
- [ ] Integrate with file system monitoring and tracking

## Security Considerations
- [ ] Validate paths are within allowed boundaries
- [ ] Prevent path traversal attacks
- [ ] Add content size limits to prevent DoS
- [ ] Implement access control based on session context
- [ ] Handle symlinks and special files securely

## Performance Optimization
- [ ] Optimize atomic write operations for large files
- [ ] Add file writing caching where appropriate
- [ ] Support streaming writes for very large content
- [ ] Monitor and optimize directory creation overhead

## Acceptance Criteria
- Complete `fs/write_text_file` method handler implementation
- File creation compliance with "MUST create" ACP requirement
- Parent directory creation when paths don't exist
- Atomic write operations ensuring file integrity
- Proper error handling for all failure scenarios
- Integration with session validation and capability checking
- Security validation for file access and path boundaries
- Performance optimization for large file writing
- Comprehensive test coverage for all scenarios
- Documentation of method behavior and requirements