# Implement ACP Working Directory Management

## Problem
Our session setup doesn't properly implement working directory management as required by the ACP specification. We need to enforce absolute path requirements, proper directory validation, and use the working directory as a boundary for file system operations.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/session-setup:

**Working Directory Rules:**
- **MUST** be an absolute path
- **MUST** be used for the session regardless of where the Agent subprocess was spawned
- **SHOULD** serve as a boundary for tool operations on the file system

## Current Issues
- May not validate that `cwd` is an absolute path
- May not enforce working directory usage throughout session
- Missing file system operation boundaries based on working directory
- No validation of directory existence or permissions

## Implementation Tasks

### Path Validation
- [ ] Validate `cwd` parameter is an absolute path
- [ ] Check directory exists and is accessible  
- [ ] Validate read/write permissions for directory
- [ ] Reject relative paths with proper error messages
- [ ] Handle path normalization across platforms (Windows/Unix)

### Directory Enforcement
- [ ] Set session working directory upon creation
- [ ] Ensure all file system operations respect session `cwd`
- [ ] Override process working directory for session context
- [ ] Store `cwd` as part of session state for persistence

### File System Boundary Implementation
- [ ] Implement path resolution relative to session `cwd`
- [ ] Add boundary checking for file operations
- [ ] Prevent access outside working directory tree (optional security)
- [ ] Handle symbolic links and path traversal safely
- [ ] Add configuration for strict vs permissive boundary enforcement

### Session State Management  
- [ ] Store working directory in session metadata
- [ ] Persist working directory for session loading
- [ ] Validate working directory exists when loading sessions
- [ ] Handle working directory changes during session lifecycle

### Platform Compatibility
- [ ] Handle Windows absolute paths (C:\, UNC paths)
- [ ] Handle Unix absolute paths (starting with /)
- [ ] Normalize path separators across platforms
- [ ] Handle case sensitivity differences
- [ ] Support network paths and mounted volumes

## Error Handling
Proper error responses for working directory issues:
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
    "message": "Working directory not accessible: permission denied",
    "data": {
      "path": "/home/user/restricted",
      "error": "permission_denied",
      "requiredPermissions": ["read", "execute"]
    }
  }
}
```

## Implementation Notes
Add working directory management comments:
```rust
// ACP requires strict working directory management:
// 1. Must be absolute path - no relative paths allowed
// 2. Must be used regardless of where agent process started  
// 3. Should serve as boundary for file system operations
// 4. Must persist across session loads
//
// All file system operations should resolve paths relative to session cwd.
```

## File System Integration
- [ ] Update file read/write operations to use session `cwd`
- [ ] Modify terminal operations to run in session `cwd`
- [ ] Ensure MCP server operations respect working directory
- [ ] Add path resolution utilities for session-relative operations

## Testing Requirements
- [ ] Test absolute path validation (Unix and Windows formats)
- [ ] Test rejection of relative paths with proper errors
- [ ] Test directory existence and permission validation
- [ ] Test file operations are bounded to working directory
- [ ] Test working directory persistence across session loads
- [ ] Test path normalization across platforms
- [ ] Test symbolic link and traversal handling
- [ ] Test error scenarios (non-existent paths, permission issues)

## Acceptance Criteria
- All working directory paths must be absolute per ACP spec
- Proper validation of directory existence and permissions
- File system operations respect session working directory boundary
- Working directory persists across session loads
- Cross-platform path handling (Windows/Unix)
- Comprehensive error handling with proper ACP error codes
- Clear error messages for path validation failures
- Security boundaries prevent unauthorized file access (configurable)
- Complete test coverage for all working directory scenarios