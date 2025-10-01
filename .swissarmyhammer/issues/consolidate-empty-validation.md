# Standardize Empty/Null Value Validation

## Problem
Empty value validation appears in 7+ files with inconsistent error handling:
- Different error types for same validation
- Inconsistent error messages
- Mix of boolean returns and detailed errors

## Locations

**Path validation** - `path_validator.rs:87-89`
```rust
if path_str.is_empty() {
    return Err(PathValidationError::EmptyPath);
}
```

**Working directory** - `request_validation.rs:76-86`
```rust
if cwd.as_os_str().is_empty() {
    return Err(SessionSetupError::InvalidParameterType(Box::new(...)));
}
```

**URI validation** - `content_security_validator.rs:407`
```rust
if uri.is_empty() {
    // Error handling
}
```

**Base64 validation** - `base64_processor.rs:345`
```rust
if data.is_empty() {
    return Err(Base64ProcessorError::InvalidBase64("Empty base64 data".to_string()));
}
```

**Session ID** - `request_validation.rs:57`
```rust
if session_id.is_empty() {
    return Err(SessionSetupError::MissingRequiredParameter { ... });
}
```

## Inconsistencies

### Three Different Approaches:

1. **Simple error variant**:
```rust
return Err(PathValidationError::EmptyPath);
```

2. **Detailed error with context**:
```rust
return Err(SessionSetupError::InvalidParameterType(Box::new(
    InvalidParameterTypeDetails {
        parameter_name: "cwd".to_string(),
        expected_type: "non-empty PathBuf".to_string(),
        actual_type: "empty path".to_string(),
        ...
    }
)));
```

3. **String-based error**:
```rust
return Err(Base64ProcessorError::InvalidBase64("Empty base64 data".to_string()));
```

## Recommendation

### Create Validation Trait
**New file:** `lib/src/validation_utils/common.rs`

```rust
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error("Field '{field}' cannot be empty")]
    EmptyField { field: String },
    
    #[error("Field '{field}' exceeds maximum length: {actual} > {max}")]
    LengthExceeded { field: String, actual: usize, max: usize },
    
    #[error("Field '{field}' contains invalid value: {reason}")]
    InvalidValue { field: String, reason: String },
}

/// Trait for validating non-empty values
pub trait ValidateNotEmpty {
    fn validate_not_empty(&self, field_name: &str) -> Result<(), ValidationError>;
}

impl ValidateNotEmpty for String {
    fn validate_not_empty(&self, field_name: &str) -> Result<(), ValidationError> {
        if self.is_empty() {
            return Err(ValidationError::EmptyField {
                field: field_name.to_string(),
            });
        }
        Ok(())
    }
}

impl ValidateNotEmpty for &str {
    fn validate_not_empty(&self, field_name: &str) -> Result<(), ValidationError> {
        if self.is_empty() {
            return Err(ValidationError::EmptyField {
                field: field_name.to_string(),
            });
        }
        Ok(())
    }
}

impl ValidateNotEmpty for Path {
    fn validate_not_empty(&self, field_name: &str) -> Result<(), ValidationError> {
        if self.as_os_str().is_empty() {
            return Err(ValidationError::EmptyField {
                field: field_name.to_string(),
            });
        }
        Ok(())
    }
}

/// Validate length against maximum
pub fn validate_length_limit(
    value_len: usize,
    max: usize,
    field_name: &str,
) -> Result<(), ValidationError> {
    if value_len > max {
        return Err(ValidationError::LengthExceeded {
            field: field_name.to_string(),
            actual: value_len,
            max,
        });
    }
    Ok(())
}
```

### Update Usage

```rust
// Old (multiple variations):
if path_str.is_empty() {
    return Err(PathValidationError::EmptyPath);
}

// New (consistent):
use validation_utils::common::ValidateNotEmpty;
path_str.validate_not_empty("path")?;
```

## Impact
- Consistent empty value validation across codebase
- Standardized error messages with field context
- Type-safe validation through trait
- Easy to extend to new types
- Reduces validation boilerplate