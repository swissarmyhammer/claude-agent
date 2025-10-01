# Consolidate Error Type Hierarchy

## Problem
The codebase has 13+ custom error types with significant duplication and inconsistency:
- Duplicated error variants (SizeExceeded, CapabilityNotSupported, ProcessingTimeout, etc.)
- Inconsistent error conversion patterns
- Manual repetitive error conversion to JSON-RPC

## Locations
### Duplicated Variants

**SizeExceeded errors:**
- `base64_processor.rs:13` - `Base64ProcessorError::SizeExceeded`
- `content_block_processor.rs:35` - `ContentBlockProcessorError::ContentSizeExceeded`
- `acp_error_conversion.rs:42` - `ContentProcessingError::ContentSizeExceeded`

**CapabilityNotSupported errors:**
- `base64_processor.rs:26` - `Base64ProcessorError::CapabilityNotSupported`
- `content_block_processor.rs:41` - `ContentBlockProcessorError::CapabilityNotSupported`
- `acp_error_conversion.rs:48` - `ContentProcessingError::CapabilityNotSupported`

**ProcessingTimeout errors:**
- `base64_processor.rs:22` - `Base64ProcessorError::ProcessingTimeout`
- `content_block_processor.rs:39` - `ContentBlockProcessorError::ProcessingTimeout`
- `acp_error_conversion.rs:54` - `ContentProcessingError::ProcessingTimeout`
- `content_security_validator.rs:37` - `ContentSecurityError::ProcessingTimeout`

### Inconsistent Conversion
- `error.rs:176-193` - `to_json_rpc_error()` method
- `session_errors.rs:205-239` - `to_json_rpc_code()` method (different name!)
- `acp_error_conversion.rs:96-873` - Manual conversion functions

## Recommendations

### 1. Create Unified Error Trait
```rust
pub trait ToJsonRpcError {
    fn to_json_rpc_code(&self) -> i32;
    fn to_json_rpc_data(&self) -> serde_json::Value;
}
```

### 2. Consolidate Common Error Variants
Create `lib/src/common_errors.rs`:
```rust
#[derive(thiserror::Error, Debug)]
pub enum CommonContentError {
    #[error("Content size exceeded: {actual} bytes (limit: {limit})")]
    SizeExceeded { actual: usize, limit: usize },
    
    #[error("Processing timeout after {elapsed_ms}ms")]
    ProcessingTimeout { elapsed_ms: u64 },
    
    #[error("Capability not supported: {capability}")]
    CapabilityNotSupported { capability: String },
}
```

### 3. Remove Duplicated Variants
Update error types to wrap CommonContentError instead of duplicating.

## Impact
- Reduces error type definitions by ~30%
- Ensures consistent error handling
- Simplifies error conversion logic
- Improves maintainability