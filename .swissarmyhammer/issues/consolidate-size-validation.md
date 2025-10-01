# Consolidate Size Validation Logic

## Problem
Size validation logic is scattered across multiple modules with inconsistent approaches:
- Configuration-driven limits
- Policy-based limits
- Hardcoded constants
- Different size calculation methods

## Locations

**Base64 size checking** - `base64_processor.rs:371-383`
```rust
fn check_size_limits(&self, data: &str) -> Result<(), Base64ProcessorError> {
    let estimated_decoded_size = (data.len() * 3) / 4;
    if estimated_decoded_size > self.max_size { ... }
}
```

**Content size checking** - `content_block_processor.rs:323-328`
```rust
if decoded_data.len() > self.max_resource_size {
    return Err(ContentBlockProcessorError::ContentSizeExceeded { ... });
}
```

**Path length validation** - `path_validator.rs:92-96`
```rust
if path_str.len() > self.max_path_length {
    return Err(PathValidationError::PathTooLong(path_str.len(), self.max_path_length));
}
```

**URI length validation** - `content_security_validator.rs:414-422`
```rust
if uri.len() > self.policy.max_uri_length {
    return Err(ContentSecurityError::UriSecurityViolation { ... });
}
```

**Meta object size** - `request_validation.rs:141-154`
```rust
if meta_str.len() > 100_000 {  // Hardcoded!
    return Err(SessionSetupError::InvalidParameterType(...));
}
```

## Issues
1. **Inconsistent limits**: Some use configuration, some hardcode values
2. **Different error types**: Each module has its own size error variant
3. **Duplicated logic**: Basic size checks repeated 5+ times
4. **No centralized policy**: Size limits scattered throughout codebase

## Recommendation

### Create Size Validation Module
**New file:** `lib/src/validation_utils/size.rs`

```rust
/// Centralized size validation with configurable limits
pub struct SizeValidator {
    limits: SizeLimits,
}

#[derive(Debug, Clone)]
pub struct SizeLimits {
    pub max_path_length: usize,
    pub max_url_length: usize,
    pub max_base64_size: usize,
    pub max_content_size: usize,
    pub max_meta_size: usize,
}

impl SizeLimits {
    pub fn default() -> Self {
        Self {
            max_path_length: 4096,
            max_url_length: 8192,
            max_base64_size: 10 * 1024 * 1024, // 10MB
            max_content_size: 50 * 1024 * 1024, // 50MB
            max_meta_size: 100_000, // 100KB
        }
    }
    
    pub fn strict() -> Self {
        Self {
            max_path_length: 2048,
            max_url_length: 2048,
            max_base64_size: 1024 * 1024, // 1MB
            max_content_size: 5 * 1024 * 1024, // 5MB
            max_meta_size: 10_000, // 10KB
        }
    }
}

impl SizeValidator {
    pub fn new(limits: SizeLimits) -> Self {
        Self { limits }
    }
    
    /// Validate generic size against limit
    pub fn validate_size(
        &self,
        actual: usize,
        limit: usize,
        field_name: &str,
    ) -> Result<(), ValidationError> {
        if actual > limit {
            return Err(ValidationError::SizeExceeded {
                field: field_name.to_string(),
                actual,
                limit,
            });
        }
        Ok(())
    }
    
    /// Validate base64 data size (estimated decoded size)
    pub fn validate_base64_size(&self, data: &str) -> Result<(), ValidationError> {
        let estimated_decoded_size = (data.len() * 3) / 4;
        self.validate_size(
            estimated_decoded_size,
            self.limits.max_base64_size,
            "base64_data"
        )
    }
    
    /// Validate path length
    pub fn validate_path_length(&self, path: &str) -> Result<(), ValidationError> {
        self.validate_size(path.len(), self.limits.max_path_length, "path")
    }
    
    /// Validate URL length
    pub fn validate_url_length(&self, url: &str) -> Result<(), ValidationError> {
        self.validate_size(url.len(), self.limits.max_url_length, "url")
    }
    
    /// Validate content size
    pub fn validate_content_size(&self, size: usize) -> Result<(), ValidationError> {
        self.validate_size(size, self.limits.max_content_size, "content")
    }
    
    /// Validate metadata object size
    pub fn validate_meta_size(&self, size: usize) -> Result<(), ValidationError> {
        self.validate_size(size, self.limits.max_meta_size, "metadata")
    }
}
```

### Update Usage

Replace hardcoded checks with validator:
```rust
// Old (request_validation.rs:141):
if meta_str.len() > 100_000 {
    return Err(...);
}

// New:
size_validator.validate_meta_size(meta_str.len())?;
```

## Impact
- Eliminates 5+ duplicated size checks
- Centralized size limit configuration
- Consistent error handling for size violations
- Easy to adjust limits globally
- Removes hardcoded magic numbers