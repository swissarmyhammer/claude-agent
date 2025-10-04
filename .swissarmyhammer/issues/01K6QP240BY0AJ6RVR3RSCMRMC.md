# Remove Content Processing Timeouts and Document Anti-Pattern

## Description
Remove all timeout handling from content processing components and replace with comments documenting why timeouts should not be used in these contexts.

## Problem
Content processing timeouts create artificial limitations and poor user experience:
- Base64 processing, content security validation, and content block processing all have arbitrary timeout limits
- These timeouts can interrupt legitimate processing of large or complex content
- Timeout handling adds complexity without meaningful benefit
- Users cannot predict when operations will be artificially terminated

## Components to Modify

### Files with timeout removal needed:
- `lib/src/base64_processor.rs:340-351` - Remove `with_timeout()` method and ProcessingTimeout error
- `lib/src/content_security_validator.rs:395-401` - Remove processing timeout validation  
- `lib/src/content_block_processor.rs:350-361` - Remove `with_timeout()` wrapper

### Specific Changes:
1. **Base64 Processor**: Remove `ProcessingTimeout` error variant and `with_timeout()` method
2. **Content Security Validator**: Remove `processing_timeout` from policy and timeout checking
3. **Content Block Processor**: Remove `processing_timeout` field and timeout wrapper
4. **Error Conversion**: Remove timeout error mappings in `acp_error_conversion.rs`

## Documentation Requirements
Add comments in each modified file explaining:
```rust
// IMPORTANT: Do not add timeouts to content processing operations.
// Content processing should be allowed to complete regardless of size or complexity.
// Timeouts create artificial limitations and poor user experience.
// See memo: [MEMO_ID] for detailed rationale.
```

## Priority
Medium - Improves user experience and removes artificial limitations

## Files Affected
- `lib/src/base64_processor.rs`
- `lib/src/content_security_validator.rs` 
- `lib/src/content_block_processor.rs`
- `lib/src/acp_error_conversion.rs`