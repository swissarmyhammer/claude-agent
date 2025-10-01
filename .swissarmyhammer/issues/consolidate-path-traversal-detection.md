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