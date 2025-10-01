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


## Proposed Solution

Based on analysis of the existing code, I will create a unified size validation module that consolidates all size-related validation logic. The approach will:

### 1. Create New Size Validator Module
**File:** `lib/src/size_validator.rs`

This module will:
- Define a `SizeValidator` struct with configurable limits
- Provide specialized validation methods for different size checks
- Return a unified `SizeValidationError` type
- Support both estimated sizes (base64) and actual sizes

Key design decisions:
- Use builder pattern for flexible configuration
- All limits configurable with sensible defaults
- Separate error type that can be converted to existing error types
- Methods return `Result<(), SizeValidationError>` for easy integration

### 2. Integration Strategy

Replace inline size checks in:
1. **base64_processor.rs:450** - `check_size_limits()` using estimated decoded size
2. **content_block_processor.rs:436,482** - Direct size checks on decoded data
3. **path_validator.rs:105** - Path length validation
4. **content_security_validator.rs:531** - URI length validation  
5. **request_validation.rs:145** - Hardcoded meta size check (100_000)

### 3. Maintain Existing Error Types

To avoid breaking changes:
- Each module keeps its existing error variants
- Add `From<SizeValidationError>` implementations for seamless conversion
- Preserve error messages and JSON-RPC error codes

### 4. Configuration Integration

Current limits from code analysis:
- `max_path_length`: 4096 (path_validator.rs:57)
- `max_base64_size`: 10MB (base64_processor.rs:167)
- `max_resource_size`: 50MB (content_block_processor.rs:221)
- `max_uri_length`: 2048/4096/8192 (content_security_validator.rs policy-dependent)
- `max_meta_size`: 100,000 bytes (request_validation.rs:145)

These will become configurable through `SizeValidator` while maintaining backwards compatibility.

### 5. Testing Approach (TDD)

Write tests first for:
1. Size validator creation with defaults
2. Each validation method (base64, content, path, uri, meta)
3. Edge cases (zero, max, max+1)
4. Error message formatting
5. Integration with existing error types

Then implement the size validator to pass these tests.

### 6. Implementation Steps

1. Create `lib/src/size_validator.rs` with failing tests
2. Implement `SizeValidator` struct and methods to pass tests
3. Update `lib/src/lib.rs` to export the new module
4. Update each consumer module one at a time:
   - Add `From<SizeValidationError>` implementation
   - Replace inline check with validator call
   - Run tests to verify no breakage
5. Remove old inline validation code
6. Run full test suite

### 7. Benefits

- Eliminates 5+ duplicated size checks
- Removes hardcoded magic number (100_000)
- Consistent size validation across codebase
- Single place to adjust limits
- Better testability
- Clearer error messages




## Implementation Complete

Successfully consolidated all size validation logic into a unified `SizeValidator` module.

### Changes Made

#### 1. New Module: `lib/src/size_validator.rs`
- Created `SizeValidator` struct with configurable `SizeLimits`
- Implemented validation methods for all size check types:
  - `validate_base64_size()` - estimates decoded size
  - `validate_content_size()` - validates content length
  - `validate_path_length()` - validates path string length
  - `validate_uri_length()` - validates URI string length
  - `validate_meta_size()` - validates metadata object size
- Added preset configurations: `default()`, `strict()`, `permissive()`
- Comprehensive test coverage (14 tests, all passing)

#### 2. Updated Modules

**base64_processor.rs (lib/src/base64_processor.rs:450)**
- Added `size_validator` field to `Base64Processor`
- Replaced `check_size_limits()` inline logic with `size_validator.validate_base64_size()`
- Updated all constructors to initialize size_validator
- Added `From<SizeValidationError>` for error conversion
- Tests: 6 passed

**content_block_processor.rs (lib/src/content_block_processor.rs:474,520)**
- Added `size_validator` field to `ContentBlockProcessor`
- Replaced 2 inline size checks with `size_validator.validate_content_size()`
- Updated all constructors (4 constructors)
- Added `From<SizeValidationError>` for error conversion
- Tests: 12 passed

**path_validator.rs (lib/src/path_validator.rs:105)**
- Added `size_validator` field to `PathValidator`
- Replaced inline path length check with `size_validator.validate_path_length()`
- Updated constructors: `new()` and `with_max_length()`
- Added `From<SizeValidationError>` for error conversion
- Tests: 10 passed

**content_security_validator.rs (lib/src/content_security_validator.rs:531)**
- Added `size_validator` field to `ContentSecurityValidator`
- Replaced inline URI length check with `size_validator.validate_uri_length()`
- Updated `new()` constructor and `Clone` implementation
- Added `From<SizeValidationError>` for error conversion
- Tests: 25 passed

**request_validation.rs (lib/src/request_validation.rs:145)**
- Converted `RequestValidator` from zero-sized marker to proper struct
- Added `size_validator` field
- Replaced hardcoded 100_000 constant with `size_validator.validate_meta_size()`
- Updated method signatures from `Self::method()` to `&self.method()`
- Updated test code to create validator instances
- Tests: 13 passed

#### 3. Module Registration
- Added `pub mod size_validator;` to `lib/src/lib.rs`

### Results

✅ **All 542 tests passing**
✅ **Eliminated 5+ duplicated size checks**
✅ **Removed hardcoded magic number (100_000)**
✅ **Consistent error handling across modules**
✅ **Single source of truth for size limits**

### Size Limits Now Configurable

Default limits (unchanged from original):
- `max_path_length`: 4096 bytes
- `max_uri_length`: 8192 bytes  
- `max_base64_size`: 10MB
- `max_content_size`: 50MB
- `max_meta_size`: 100KB

All limits can now be adjusted through `SizeLimits` configuration without modifying multiple files.




## Code Review Fixes Complete

All clippy linting errors identified in the code review have been fixed:

### Fixed Issues

1. **field_reassign_with_default errors (10 occurrences)**
   - Replaced pattern of creating mutable default and reassigning fields
   - Used struct literal syntax with spread operator instead
   - Files fixed:
     - base64_processor.rs (4 locations)
     - content_block_processor.rs (4 locations)
     - content_security_validator.rs (1 location)
     - path_validator.rs (1 location)

2. **redundant_pattern_matching error (1 occurrence)**
   - Replaced `if let Err(_) = ...` with `.is_err()` method
   - File fixed: request_validation.rs:161

3. **Test compilation errors (2 occurrences)**
   - Updated test code to use instance methods instead of static methods
   - Created `RequestValidator::new()` instances in tests
   - Files fixed: request_validation.rs (2 test functions)

### Verification

✅ `cargo clippy -- -D warnings` - Passes with no errors
✅ `cargo nextest run` - All 542 tests passing

### Design Decision: Struct Literal Pattern

Changed from:
```rust
let mut size_limits = crate::size_validator::SizeLimits::default();
size_limits.max_base64_size = max_size;
let size_validator = SizeValidator::new(size_limits);
```

To:
```rust
let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
    max_base64_size: max_size,
    ..Default::default()
});
```

**Rationale:**
- More idiomatic Rust - uses struct literal syntax
- Clearer intent - shows exactly which field is being customized
- Eliminates clippy warning about field reassignment after default
- More concise - reduces from 3 lines to 4 lines (with formatting)
- Immutable - avoids unnecessary mutability

This pattern is now consistent across all constructors that create custom SizeLimits.
