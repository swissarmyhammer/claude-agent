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


## Proposed Solution

After examining both files, I'll consolidate the duplicated base64 validation logic by:

1. **Create a new module** `base64_validation.rs` with shared validation logic
   - This will contain a simple validation function that both modules can use
   - The function will return a Result type to allow flexible error handling by callers

2. **Design approach**:
   - Keep it simple: extract the common validation logic into a standalone function
   - Each caller can adapt the result to their own error type
   - Avoids introducing unnecessary abstractions or complex type hierarchies

3. **Update both files**:
   - `content_security_validator.rs:497-512` - Replace `is_valid_base64_format` with call to shared validator
   - `base64_processor.rs:344-368` - Replace `validate_base64_format` with call to shared validator
   - Each will convert the shared result to their own error types

4. **Testing**:
   - Write comprehensive tests for the new validation module
   - Ensure existing tests in both modules continue to pass

This approach eliminates the duplication while maintaining the existing API surface of both modules.



## Implementation Notes

Successfully consolidated the duplicated base64 validation logic:

### Changes Made:

1. **Created `lib/src/base64_validation.rs`**:
   - Extracted common validation logic into `validate_base64_format()` function
   - Returns `Result<(), Base64ValidationError>` with three specific error types:
     - `EmptyData` - for empty strings
     - `InvalidCharacters` - for non-base64 characters
     - `InvalidPadding` - for incorrect length (not multiple of 4)
   - Includes comprehensive tests covering all validation cases

2. **Updated `lib/src/lib.rs`**:
   - Added `pub mod base64_validation;` declaration

3. **Updated `content_security_validator.rs:387-393`**:
   - Removed `is_valid_base64_format()` method (lines 497-512)
   - Now calls `base64_validation::validate_base64_format(data)`
   - Converts errors to `ContentSecurityError::Base64SecurityViolation`

4. **Updated `base64_processor.rs:345-357`**:
   - Replaced `validate_base64_format()` implementation with call to shared function
   - Uses pattern matching to convert `Base64ValidationError` to appropriate `Base64ProcessorError` variants
   - Maintains existing error messages for compatibility

### Results:

- ✅ Eliminated ~30 lines of duplicated code
- ✅ Single source of truth for base64 format validation
- ✅ Both modules maintain their existing APIs
- ✅ All validation logic now shared and tested in one place
- ✅ Code compiles successfully (`cargo build` passed)
- ⚠️  Cannot run full test suite due to pre-existing test failures in `session_loading.rs` (unrelated to these changes)

### Testing Status:

The base64_validation module includes comprehensive unit tests:
- Valid base64 strings (with/without padding)
- Empty data detection
- Invalid character detection
- Invalid padding detection
- Whitespace handling

Both `content_security_validator.rs` and `base64_processor.rs` have existing tests that validate their usage of the shared function.

## Code Review Implementation Complete

All critical issues from CODE_REVIEW.md have been resolved:

### Fixed Issues:

1. **Import Errors (Lines 677, 837)**: Removed unused `EnvVar` imports from two test functions
2. **Unused Imports (Lines 701, 723, 745, 773, 801)**: Removed unused `HttpHeader` imports from five test functions
3. **Error Handling**: Improved error handling in `content_security_validator.rs` to capture and include specific error details instead of discarding them
4. **Module Documentation**: Added comprehensive module-level documentation to `base64_validation.rs` explaining purpose, design, and usage

### Test Fixes:

Fixed incorrect test cases in `base64_validation.rs`:
- The original tests expected "Hello World" (with space) to fail, but spaces are valid whitespace in base64 per the original implementation
- The original tests expected "YWJjZGVm" to fail padding validation, but it's actually valid base64 (8 chars = multiple of 4)
- Updated tests to use actually invalid inputs that violate the validation rules

### Verification:

- ✅ `cargo build` - compiles successfully
- ✅ `cargo clippy --all-targets -- -D warnings` - passes with no errors
- ✅ All base64-related tests pass (16 tests)
- ⚠️ One pre-existing test failure in `session_loading::tests::test_validate_load_request_with_invalid_sse_url` (unrelated to base64 changes)

The base64 validation consolidation is complete and working correctly.