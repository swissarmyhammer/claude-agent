# Implement MIME Type Validation and Handling

## Problem
Our content processing may not properly validate and handle MIME types as required by the ACP specification. We need comprehensive MIME type validation, security checking, and proper handling for all content block types.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/content:

**MIME Type Requirements by Content Type:**
- **Image Content**: `image/png`, `image/jpeg`, `image/gif`, `image/webp`, etc.
- **Audio Content**: `audio/wav`, `audio/mp3`, `audio/ogg`, `audio/aac`, etc.
- **Embedded Resources**: Any MIME type (`text/plain`, `application/json`, `text/x-python`, etc.)
- **Resource Links**: Any MIME type for referenced resources

**Example Usage:**
```json
{
  "type": "image",
  "mimeType": "image/png",
  "data": "base64data..."
}
```

## Current Issues
- MIME type validation implementation unclear
- No security restrictions on allowed MIME types
- Missing format validation against actual content
- No proper error handling for unsupported MIME types

## Implementation Tasks

### MIME Type Validation Infrastructure
- [ ] Create comprehensive MIME type validation system
- [ ] Add MIME type format validation (proper structure)
- [ ] Implement allowlists for supported MIME types by content type
- [ ] Add security filtering for potentially dangerous MIME types

### Image MIME Type Support
- [ ] Support standard image MIME types: `image/png`, `image/jpeg`, `image/gif`, `image/webp`
- [ ] Validate image MIME types against actual image format
- [ ] Add security restrictions for image types
- [ ] Handle edge cases and lesser-used image formats

### Audio MIME Type Support  
- [ ] Support standard audio MIME types: `audio/wav`, `audio/mp3`, `audio/ogg`, `audio/aac`
- [ ] Validate audio MIME types against actual audio format
- [ ] Add security restrictions for audio types
- [ ] Handle compressed vs uncompressed audio formats

### Resource MIME Type Support
- [ ] Support text MIME types: `text/plain`, `text/html`, `text/css`, `text/javascript`
- [ ] Support code MIME types: `text/x-python`, `text/x-rust`, `application/json`, `application/xml`
- [ ] Support document MIME types: `application/pdf`, `application/msword`, `text/markdown`
- [ ] Add flexible MIME type support for embedded resources

### MIME Type Security Validation
- [ ] Implement security allowlists for MIME types
- [ ] Block potentially dangerous MIME types (executables, etc.)
- [ ] Add content sniffing to detect MIME type spoofing
- [ ] Validate MIME type matches actual content format

## MIME Type Validation Implementation
```rust
pub struct MimeTypeValidator {
    allowed_image_types: HashSet<String>,
    allowed_audio_types: HashSet<String>,
    blocked_types: HashSet<String>,
    require_format_validation: bool,
}

impl MimeTypeValidator {
    pub fn validate_image_mime_type(&self, mime_type: &str, data: &[u8]) -> Result<()> {
        // Check if MIME type is allowed for images
        if !self.allowed_image_types.contains(mime_type) {
            return Err(ValidationError::UnsupportedMimeType(mime_type.to_string()));
        }
        
        // Validate actual format matches declared MIME type
        if self.require_format_validation {
            self.validate_image_format_matches_mime(data, mime_type)?;
        }
        
        Ok(())
    }
    
    pub fn validate_audio_mime_type(&self, mime_type: &str, data: &[u8]) -> Result<()>;
    pub fn validate_resource_mime_type(&self, mime_type: &str) -> Result<()>;
    pub fn is_mime_type_secure(&self, mime_type: &str) -> bool;
}
```

## Implementation Notes
Add MIME type validation comments:
```rust
// ACP requires comprehensive MIME type validation and security:
// 1. Image: Validate against supported image formats
// 2. Audio: Validate against supported audio formats
// 3. Resources: Allow flexible MIME types with security filtering
// 4. Security: Block dangerous MIME types and validate format matching
// 5. Format validation: Ensure declared MIME type matches actual content
//
// MIME type validation prevents security issues and ensures proper content handling.
```

### Format Detection and Validation
- [ ] Implement content format detection from binary data
- [ ] Compare detected format with declared MIME type
- [ ] Handle MIME type spoofing attempts
- [ ] Support format detection for common types
- [ ] Add fuzzy matching for similar formats

### Security Filtering
```rust
const BLOCKED_MIME_TYPES: &[&str] = &[
    "application/x-executable",
    "application/x-msdownload", 
    "application/x-msdos-program",
    "text/html", // Potentially dangerous in some contexts
    "application/javascript", // Potentially dangerous
];

const ALLOWED_IMAGE_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg", 
    "image/gif",
    "image/webp",
    "image/bmp",
    "image/svg+xml", // With additional XML validation
];
```

### Error Handling and Response
- [ ] Return proper error responses for unsupported MIME types
- [ ] Include allowed MIME types in error messages
- [ ] Handle MIME type format validation errors
- [ ] Provide security-related error messages

### Configuration and Customization
- [ ] Add configurable MIME type allowlists
- [ ] Support different validation levels (strict, permissive)
- [ ] Configure security filtering policies
- [ ] Add runtime MIME type configuration updates

## Error Response Examples
For unsupported image MIME type:
```json
{
  "error": {
    "code": -32602,
    "message": "Unsupported MIME type for image content: image/tiff",
    "data": {
      "providedMimeType": "image/tiff",
      "contentType": "image",
      "allowedTypes": ["image/png", "image/jpeg", "image/gif", "image/webp"],
      "suggestion": "Convert image to supported format"
    }
  }
}
```

For security-blocked MIME type:
```json
{
  "error": {
    "code": -32602,
    "message": "MIME type blocked for security reasons: application/x-executable", 
    "data": {
      "providedMimeType": "application/x-executable",
      "reason": "executable_content_blocked",
      "allowedCategories": ["image", "audio", "text", "document"]
    }
  }
}
```

## Testing Requirements
- [ ] Test MIME type validation for all supported content types
- [ ] Test security filtering blocks dangerous MIME types
- [ ] Test format validation matches declared MIME types
- [ ] Test error responses for unsupported MIME types
- [ ] Test MIME type spoofing detection
- [ ] Test configuration of MIME type allowlists
- [ ] Test edge cases and malformed MIME type strings
- [ ] Test performance with large numbers of MIME type validations

## Integration Points
- [ ] Connect to content block validation system
- [ ] Integrate with base64 data processing
- [ ] Connect to security validation pipeline
- [ ] Integrate with error response system

## Performance Considerations
- [ ] Optimize MIME type validation for frequent operations
- [ ] Cache validation results for repeated MIME types
- [ ] Support batch MIME type validation
- [ ] Minimize overhead in content processing pipeline

## Acceptance Criteria
- Comprehensive MIME type validation for all content types
- Security filtering blocks dangerous MIME types
- Format validation ensures MIME type matches actual content
- Proper error responses for unsupported MIME types
- Configurable MIME type allowlists and security policies
- Performance optimization for MIME type validation
- Complete test coverage for all MIME type scenarios
- Integration with existing content processing systems
- Clear error messages explaining MIME type requirements