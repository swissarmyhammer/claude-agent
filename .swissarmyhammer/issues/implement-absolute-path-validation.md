# Implement Absolute Path Validation for File Operations

## Problem
Our file system operations may not properly validate that file paths are absolute as required by the ACP specification. All file system methods require absolute paths, and we need comprehensive path validation and normalization across platforms.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/file-system:

**Path Requirements:**
- All file paths MUST be absolute paths
- No relative path support allowed
- Cross-platform path handling (Windows vs Unix)
- Proper path validation and normalization

**Examples:**
- ✅ Valid: `/home/user/project/src/main.py` (Unix)
- ✅ Valid: `C:\Users\user\project\src\main.py` (Windows)
- ❌ Invalid: `./src/main.py` (relative)
- ❌ Invalid: `../config.json` (relative)
- ❌ Invalid: `src/main.py` (relative)

## Current Issues
- Path validation against absolute path requirement unclear
- Missing cross-platform path handling
- No comprehensive path normalization
- Missing security validation for path traversal

## Implementation Tasks

### Path Validation Infrastructure
- [ ] Create comprehensive path validation system
- [ ] Add absolute path detection for Unix and Windows
- [ ] Implement path format validation and normalization
- [ ] Add cross-platform path compatibility

### Unix Path Validation
- [ ] Validate paths start with `/` for Unix systems
- [ ] Handle Unix path normalization and canonicalization
- [ ] Support Unix symbolic links and path resolution
- [ ] Add Unix-specific path security validation

### Windows Path Validation
- [ ] Validate Windows absolute paths (C:\ format, UNC paths)
- [ ] Handle Windows path normalization (forward/back slashes)
- [ ] Support Windows drive letters and UNC network paths
- [ ] Add Windows-specific path security validation

### Path Security and Sanitization
- [ ] Prevent path traversal attacks (../, ..\, etc.)
- [ ] Validate paths are within allowed boundaries
- [ ] Handle symbolic links and junction points securely
- [ ] Add path length limits and character validation

## Path Validation Implementation
```rust
use std::path::{Path, PathBuf};

pub struct PathValidator {
    allowed_roots: Vec<PathBuf>,
    max_path_length: usize,
}

impl PathValidator {
    pub fn validate_absolute_path(&self, path_str: &str) -> Result<PathBuf, PathValidationError> {
        // Check path length
        if path_str.len() > self.max_path_length {
            return Err(PathValidationError::PathTooLong(path_str.len()));
        }
        
        // Parse and validate path
        let path = PathBuf::from(path_str);
        
        // Check if path is absolute
        if !path.is_absolute() {
            return Err(PathValidationError::NotAbsolute(path_str.to_string()));
        }
        
        // Normalize path (resolve . and .. components)
        let normalized = self.normalize_path(&path)?;
        
        // Check for path traversal attempts
        self.validate_path_security(&normalized)?;
        
        // Validate path is within allowed boundaries
        self.validate_path_boundaries(&normalized)?;
        
        Ok(normalized)
    }
    
    fn normalize_path(&self, path: &Path) -> Result<PathBuf, PathValidationError> {
        // Canonicalize path to resolve symlinks and normalize
        path.canonicalize()
            .map_err(|e| PathValidationError::CanonicalizationFailed(
                path.to_string_lossy().to_string(), e
            ))
    }
    
    fn validate_path_security(&self, path: &Path) -> Result<(), PathValidationError> {
        // Check for dangerous path components
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(PathValidationError::PathTraversalAttempt);
                }
                std::path::Component::CurDir => {
                    return Err(PathValidationError::RelativeComponent);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PathValidationError {
    #[error("Path is not absolute: {0}")]
    NotAbsolute(String),
    
    #[error("Path traversal attempt detected")]
    PathTraversalAttempt,
    
    #[error("Path contains relative components")]
    RelativeComponent,
    
    #[error("Path too long: {0} > maximum allowed")]
    PathTooLong(usize),
    
    #[error("Path canonicalization failed for {0}: {1}")]
    CanonicalizationFailed(String, std::io::Error),
    
    #[error("Path outside allowed boundaries: {0}")]
    OutsideBoundaries(String),
}
```

## Implementation Notes
Add absolute path validation comments:
```rust
// ACP requires strict absolute path validation:
// 1. All paths MUST be absolute (no relative paths allowed)
// 2. Unix: Must start with / 
// 3. Windows: Must include drive letter (C:\) or UNC path
// 4. Path traversal prevention (no ../ components)
// 5. Cross-platform normalization and security validation
//
// Path validation prevents security issues and ensures protocol compliance.
```

### Platform-Specific Validation
```rust
#[cfg(unix)]
fn is_absolute_unix(path: &str) -> bool {
    path.starts_with('/')
}

#[cfg(windows)]
fn is_absolute_windows(path: &str) -> bool {
    // Check for drive letter format (C:\)
    if path.len() >= 3 && path.chars().nth(1) == Some(':') && path.chars().nth(2) == Some('\\') {
        return true;
    }
    
    // Check for UNC path (\\server\share)
    if path.starts_with(r"\\") && path.len() > 2 {
        return true;
    }
    
    false
}

pub fn validate_absolute_path_platform(path: &str) -> Result<(), PathValidationError> {
    #[cfg(unix)]
    {
        if !is_absolute_unix(path) {
            return Err(PathValidationError::NotAbsolute(path.to_string()));
        }
    }
    
    #[cfg(windows)]
    {
        if !is_absolute_windows(path) {
            return Err(PathValidationError::NotAbsolute(path.to_string()));
        }
    }
    
    Ok(())
}
```

### Path Boundary Validation
- [ ] Define allowed root directories for file operations
- [ ] Validate paths are within workspace boundaries
- [ ] Support configurable path restrictions
- [ ] Add session-specific path boundaries

### Error Handling and Responses
- [ ] Return proper ACP error codes for invalid paths
- [ ] Provide clear error messages explaining path requirements
- [ ] Include path validation suggestions in error responses
- [ ] Handle path validation errors gracefully

## Error Response Examples
For relative path:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid path: must be absolute path",
    "data": {
      "providedPath": "./src/main.py",
      "error": "relative_path_not_allowed",
      "requirement": "absolute_path",
      "examples": {
        "unix": "/home/user/project/src/main.py",
        "windows": "C:\\Users\\user\\project\\src\\main.py"
      }
    }
  }
}
```

For path traversal attempt:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid path: path traversal detected",
    "data": {
      "providedPath": "/home/user/../../../etc/passwd",
      "error": "path_traversal_attempt",
      "reason": "Contains parent directory references (..)"
    }
  }
}
```

## Testing Requirements
- [ ] Test Unix absolute path validation (starts with /)
- [ ] Test Windows absolute path validation (drive letters, UNC paths)
- [ ] Test relative path rejection with proper errors
- [ ] Test path traversal prevention (../, ..\, etc.)
- [ ] Test path normalization and canonicalization
- [ ] Test path boundary validation
- [ ] Test cross-platform path handling
- [ ] Test edge cases (empty paths, root paths, etc.)

## Integration Points
- [ ] Connect to file system method handlers
- [ ] Integrate with session validation system
- [ ] Connect to security and access control systems
- [ ] Integrate with error handling and response systems

## Configuration Support
- [ ] Add configurable allowed root directories
- [ ] Support session-specific path restrictions
- [ ] Configure maximum path length limits
- [ ] Add platform-specific validation settings

## Performance Considerations
- [ ] Optimize path validation for frequent operations
- [ ] Cache path validation results where appropriate
- [ ] Support efficient path normalization
- [ ] Minimize validation overhead in file operations

## Acceptance Criteria
- Comprehensive absolute path validation for all file operations
- Cross-platform support for Unix and Windows path formats
- Path traversal prevention and security validation
- Proper ACP error responses for invalid paths
- Integration with existing file system methods
- Configurable path boundaries and restrictions
- Performance optimization for path validation overhead
- Comprehensive test coverage for all path scenarios
- Clear error messages explaining path requirements
- Documentation of path validation rules and examples