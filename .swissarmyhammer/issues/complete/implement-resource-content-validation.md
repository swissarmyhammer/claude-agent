# Implement Resource Content Security Validation

## Description
Implement full resource content security validation. Currently has placeholder logic.

## Locations
- `lib/src/content_security_validator.rs:297` - Main validation
- `lib/src/content_security_validator.rs:485` - Content sniffing
- `lib/src/content_block_processor.rs:266` - Structure validation

## Code Context
```rust
"Resource content security validation - placeholder for future implementation"

// This is a placeholder for content sniffing implementation

debug!("Resource content structure validation placeholder");
```

## Implementation Notes
- Implement content sniffing for MIME type detection
- Add security checks for resource content
- Validate content structure
- Add malicious content detection
- Implement file magic number checking
- Add comprehensive test coverage


## Proposed Solution

I will implement comprehensive resource content security validation using the following approach:

### 1. Add Content Sniffing with File Magic Numbers
- Add the `infer` crate for MIME type detection via magic numbers
- Implement `sniff_content_type()` method in `ContentSecurityValidator`
- Verify declared MIME types match actual content structure
- Support common file types: images (PNG, JPEG, GIF, WebP), audio (WAV, MP3, OGG), documents (PDF)

### 2. Enhance Resource Content Validation
- Replace placeholder at line 297 with full resource validation logic
- Validate resource URI if present
- Check resource text/blob content structure
- Apply size limits and security checks

### 3. Implement Content Type Consistency Validation
- Replace placeholder at line 485 (validate_content_type_consistency)
- Decode base64 data headers (first 512 bytes)
- Use magic number detection to verify MIME type
- Detect spoofing attempts (declared vs actual type mismatch)

### 4. Add Resource Structure Validation
- Replace placeholder at line 266 in content_block_processor.rs
- Validate Resource content structure (text vs blob variants)
- Check for required fields based on resource type
- Apply proper size limits

### 5. Test Coverage
- Test valid content type matching
- Test content type spoofing detection
- Test malicious content patterns
- Test resource structure validation
- Test edge cases (empty data, corrupted headers)

### Implementation Steps
1. Add `infer` crate dependency to lib/Cargo.toml
2. Implement content sniffing in content_security_validator.rs
3. Enhance resource validation logic
4. Update content_block_processor.rs resource handling
5. Add comprehensive test suite
6. Run tests to verify functionality


## Implementation Notes

During implementation, discovered that `EmbeddedResource.resource` is of type `EmbeddedResourceResource` enum with two variants:
- `TextResourceContents` - contains URI and text
- `BlobResourceContents` - contains URI, blob, and mimeType

Updated implementation to pattern match on these enum variants instead of using generic `.get()` accessors.


## Implementation Complete

Successfully implemented full resource content security validation:

### Completed Tasks
1. ✅ Added `infer` crate v0.16 for MIME type detection via magic numbers
2. ✅ Implemented `sniff_content_type()` method using infer library
3. ✅ Implemented full `validate_content_type_consistency()` with base64 decoding and magic number checking
4. ✅ Implemented `validate_resource_content()` with pattern matching on TextResourceContents and BlobResourceContents
5. ✅ Implemented `validate_resource_structure()` in content_block_processor
6. ✅ Added comprehensive test coverage (13 new tests)

### Key Features
- Content type sniffing using file magic numbers (supports PNG, JPEG, GIF, WebP, WAV, MP3, etc.)
- Content type spoofing detection (declared vs actual MIME type verification)
- Resource structure validation for both text and blob variants
- URI validation with SSRF protection
- Base64 data security validation
- Text content sanitization checks

### Test Results
- All 13 new tests passing
- Total: 585/586 tests passing
- One pre-existing test failure unrelated to these changes (test_audio_content_security_validation)

### Code Locations
- lib/src/content_security_validator.rs:659 - sniff_content_type implementation
- lib/src/content_security_validator.rs:668 - validate_content_type_consistency implementation
- lib/src/content_security_validator.rs:612 - validate_resource_content implementation
- lib/src/content_block_processor.rs:624 - validate_resource_structure implementation


## Code Review Implementation - 2025-10-01

### Changes Made

1. **Fixed collapsible-if lint warning** (lib/src/content_security_validator.rs:626)
   - Collapsed nested if statements into a single condition
   - Changed from nested `if !text_resource.text.is_empty() { if self.policy.enable_content_sanitization { ... } }`
   - To combined `if !text_resource.text.is_empty() && self.policy.enable_content_sanitization { ... }`

2. **Added comprehensive rustdoc documentation** for all new public methods:
   - `validate_resource_content()`: Documented validation flow, arguments, returns, and checks for both text and blob resources
   - `sniff_content_type()`: Documented magic number detection using the infer crate
   - `validate_content_type_consistency()`: Documented spoofing detection with implementation details about the 512-byte sampling
   - `normalize_mime_type()`: Documented canonical form conversion with examples

### Verification

- ✅ **cargo clippy**: All warnings resolved (0 warnings)
- ✅ **cargo nextest**: 604/605 tests passing
  - 1 pre-existing failure: `test_audio_content_security_validation` (unrelated to these changes)
  - All 13 new resource validation tests passing

### Implementation Status

✅ All code review issues addressed
✅ Lint compliance achieved
✅ Documentation complete
✅ Tests verified