# Consolidate Path Traversal Detection

## Problem
Path traversal detection has two different implementations in the same file with different approaches.

## Locations

**String-based detection** - `path_validator.rs:140-148`
```rust
fn validate_path_security_raw(&self, path_str: &str) -> Result<(), PathValidationError> {
    let suspicious_patterns = ["/../", "\\..\\", "/..", "\\..", "../", "..\\"];
    for pattern in &suspicious_patterns {
        if path_str.contains(pattern) {
            return Err(PathValidationError::PathTraversalAttempt);
        }
    }
    Ok(())
}
```

**Component-based detection** - `path_validator.rs:162-175`
```rust
fn validate_path_security(&self, path: &Path) -> Result<(), PathValidationError> {
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
```

## Issues
1. **Two different approaches** for same validation goal
2. **String-based is fragile** - can miss edge cases
3. **Component-based is more robust** but only works on Path
4. **Unclear which to use** in different contexts

## Recommendation

### Unify Path Traversal Detection
Consolidate into single implementation using component-based approach as canonical:

```rust
impl PathValidator {
    /// Primary path security validation (component-based, most robust)
    pub fn validate_path_security(&self, path: &Path) -> Result<(), PathValidationError> {
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    return Err(PathValidationError::PathTraversalAttempt {
                        path: path.display().to_string(),
                    });
                }
                std::path::Component::CurDir => {
                    return Err(PathValidationError::RelativeComponent {
                        path: path.display().to_string(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
    
    /// String-based pre-validation (faster early check)
    /// Use before parsing to Path for performance
    pub fn quick_traversal_check(&self, path_str: &str) -> Result<(), PathValidationError> {
        // Keep as optimization for early rejection
        let suspicious_patterns = ["/../", "\\..\\", "/..", "\\..", "../", "..\\"];
        for pattern in suspicious_patterns {
            if path_str.contains(pattern) {
                return Err(PathValidationError::PathTraversalAttempt {
                    path: path_str.to_string(),
                });
            }
        }
        Ok(())
    }
    
    /// Full validation pipeline (recommended)
    pub fn validate_path_full(&self, path_str: &str) -> Result<PathBuf, PathValidationError> {
        // 1. Quick string check (fast rejection)
        self.quick_traversal_check(path_str)?;
        
        // 2. Parse to Path
        let path = Path::new(path_str);
        
        // 3. Component-based security check (canonical)
        self.validate_path_security(path)?;
        
        // 4. Additional checks (length, format, etc.)
        // ...
        
        Ok(path.to_path_buf())
    }
}
```

### Update Error Types
Add context to errors:
```rust
#[derive(thiserror::Error, Debug)]
pub enum PathValidationError {
    #[error("Path traversal attempt detected in: {path}")]
    PathTraversalAttempt { path: String },
    
    #[error("Relative path component not allowed in: {path}")]
    RelativeComponent { path: String },
}
```

### Documentation
Add clear documentation on when to use each method:
```rust
/// # Path Security Validation
/// 
/// This module provides layered path validation:
/// 
/// - `quick_traversal_check()`: Fast string-based pre-check (optional optimization)
/// - `validate_path_security()`: Canonical component-based check (always use)
/// - `validate_path_full()`: Complete validation pipeline (recommended)
/// 
/// ## Recommendation
/// Use `validate_path_full()` in most cases. Only use individual methods
/// when you need fine-grained control over the validation process.
```

## Impact
- Single canonical validation method
- Clear guidance on which method to use
- Component-based check is more robust
- String check kept as optimization
- Better error context with path information


## Proposed Solution

After analyzing the code, I will consolidate the path traversal detection with the following approach:

### 1. Update Error Types
Add path context to `PathTraversalAttempt` and `RelativeComponent` errors:
```rust
#[error("Path traversal attempt detected in: {0}")]
PathTraversalAttempt(String),

#[error("Path contains relative components: {0}")]
RelativeComponent(String),
```

### 2. Rename and Document Methods
- Rename `validate_path_security_raw` to `quick_traversal_check` - clearly indicating it's an optimization for early rejection
- Keep `validate_path_security` as the canonical component-based check
- Add comprehensive documentation explaining the purpose and usage of each method

### 3. Implementation Strategy
- The string-based check (`quick_traversal_check`) runs first in `validate_absolute_path` for fast rejection
- The component-based check (`validate_path_security`) runs after canonicalization as the authoritative validation
- Both checks remain because they serve different purposes:
  - String check: Fast early rejection before expensive operations
  - Component check: Canonical validation after path normalization

### 4. Update All Callers
- Update `validate_absolute_path` to pass path strings to error constructors
- Update all tests to expect new error format with path context

This approach:
- Makes the purpose of each method clear
- Keeps both checks as they serve complementary roles
- Provides better error messages with path context
- Maintains backward compatibility in validation behavior



## Implementation Notes

### Changes Made

1. **Updated Error Types** (lib/src/path_validator.rs:22-26)
   - Changed `PathTraversalAttempt` from unit variant to tuple variant with `String` field
   - Changed `RelativeComponent` from unit variant to tuple variant with `String` field
   - Error messages now include the path that triggered the error

2. **Renamed `validate_path_security_raw` to `quick_traversal_check`** (lib/src/path_validator.rs:140-156)
   - Added comprehensive documentation explaining its purpose as an optimization
   - Clarified that it's NOT the canonical security check
   - Updated to return new error format with path context

3. **Enhanced `validate_path_security` Documentation** (lib/src/path_validator.rs:168-192)
   - Added detailed documentation explaining it's the canonical/authoritative check
   - Explained that component-based checking is robust against encoding tricks
   - Updated to return new error format with path context

4. **Improved `validate_absolute_path` Documentation** (lib/src/path_validator.rs:85-98)
   - Added comprehensive documentation explaining the layered validation approach
   - Numbered the 5 validation steps for clarity
   - Clarified the relationship between quick check and canonical check

5. **Updated Error Handling** (lib/src/tools.rs:1483-1492)
   - Updated pattern matches to destructure path from error variants
   - Enhanced error messages to include the specific path that failed validation

6. **Fixed All Test Cases** (lib/src/path_validator.rs:various)
   - Updated all pattern matches in tests to use tuple variant syntax
   - All 10 tests pass successfully

### Result

The consolidation is complete with:
- Clear naming that indicates purpose (quick check vs canonical check)
- Comprehensive documentation explaining when/why to use each method
- Better error messages with path context
- Both methods remain as they serve complementary purposes in the validation pipeline
- All tests passing

The code now clearly communicates that:
- `quick_traversal_check` is an optimization for fast rejection
- `validate_path_security` is the canonical component-based security check
- Both are part of a layered security approach
