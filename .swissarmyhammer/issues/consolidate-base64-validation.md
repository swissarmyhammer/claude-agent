# Consolidate Duplicated Base64 Validation Logic

## Problem
Base64 format validation logic is duplicated in two files with nearly identical code (~30 lines total duplication).

## Locations

**Location 1:** `content_security_validator.rs:497-512`
```rust
fn is_valid_base64_format(&self, data: &str) -> bool {
    if data.is_empty() {
        return false;
    }
    
    if !data.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c.is_whitespace()
    }) {
        return false;
    }
    
    let trimmed = data.trim();
    trimmed.len().is_multiple_of(4)
}
```

**Location 2:** `base64_processor.rs:344-368`
```rust
fn validate_base64_format(&self, data: &str) -> Result<(), Base64ProcessorError> {
    if data.is_empty() {
        return Err(Base64ProcessorError::InvalidBase64("Empty base64 data".to_string()));
    }
    
    if !data.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c.is_whitespace()
    }) {
        return Err(Base64ProcessorError::InvalidBase64("Contains invalid characters".to_string()));
    }
    
    let trimmed = data.trim();
    if !trimmed.len().is_multiple_of(4) {
        return Err(Base64ProcessorError::InvalidBase64("Invalid base64 padding".to_string()));
    }
    
    Ok(())
}
```

## Recommendation

### Create Shared Base64Validator Module
**New file:** `lib/src/validation_utils/base64.rs`

```rust
pub struct Base64Validator {
    max_size: usize,
    strict_format: bool,
}

impl Base64Validator {
    pub fn validate_format(&self, data: &str) -> Result<(), ValidationError> {
        if data.is_empty() {
            return Err(ValidationError::EmptyData);
        }
        
        if !data.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c.is_whitespace()
        }) {
            return Err(ValidationError::InvalidCharacters);
        }
        
        let trimmed = data.trim();
        if !trimmed.len().is_multiple_of(4) {
            return Err(ValidationError::InvalidPadding);
        }
        
        Ok(())
    }
    
    pub fn validate_size(&self, data: &str) -> Result<(), ValidationError> {
        let estimated_size = (data.len() * 3) / 4;
        if estimated_size > self.max_size {
            return Err(ValidationError::SizeExceeded { 
                actual: estimated_size, 
                limit: self.max_size 
            });
        }
        Ok(())
    }
}
```

### Update Both Files
Replace duplicated logic with calls to shared validator.

## Impact
- Eliminates 30 lines of duplication
- Single source of truth for base64 validation
- Easier to maintain and test
- Consistent validation behavior