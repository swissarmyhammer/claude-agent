# Implement Base64 Data Handling for Content Blocks

## Problem
Our content block implementation may not properly handle base64-encoded data required for image and audio content blocks, as well as blob embedded resources. We need comprehensive base64 encoding/decoding with proper validation and security measures.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/content:

**Base64 Data Requirements:**
- **Image Content**: `data` field contains base64-encoded image data
- **Audio Content**: `data` field contains base64-encoded audio data
- **Blob Resources**: `blob` field contains base64-encoded binary data

**Example Structures:**
```json
{
  "type": "image",
  "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB...",
  "mimeType": "image/png"
}
```

```json
{
  "type": "audio", 
  "data": "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAAB...",
  "mimeType": "audio/wav"
}
```

```json
{
  "type": "resource",
  "resource": {
    "uri": "file:///data/image.png",
    "blob": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB...",
    "mimeType": "image/png"
  }
}
```

## Current Issues
- Base64 encoding/decoding implementation unclear
- Missing validation of base64 data integrity
- No size limits or security validation for base64 content
- Missing proper error handling for malformed base64 data

## Implementation Tasks

### Base64 Processing Infrastructure
- [ ] Implement robust base64 encoding/decoding utilities
- [ ] Add base64 data validation and integrity checking
- [ ] Support streaming base64 processing for large data
- [ ] Add base64 padding and format validation

### Image Data Handling
- [ ] Implement base64 image data decoding
- [ ] Add image format validation after decoding
- [ ] Support multiple image formats (PNG, JPEG, GIF, WebP)
- [ ] Add image dimension and size validation
- [ ] Implement image data security checks

### Audio Data Handling
- [ ] Implement base64 audio data decoding
- [ ] Add audio format validation after decoding
- [ ] Support multiple audio formats (WAV, MP3, OGG, AAC)
- [ ] Add audio duration and size validation
- [ ] Implement audio data security checks

### Blob Resource Handling
- [ ] Handle base64 blob data in embedded resources
- [ ] Support arbitrary binary data types
- [ ] Add blob size and format validation
- [ ] Implement blob data security and sanitization

### Data Size Limits and Validation
- [ ] Implement configurable size limits for base64 data
- [ ] Add memory usage monitoring for large data processing
- [ ] Support streaming validation for large base64 content
- [ ] Add size limit enforcement before processing

## Base64 Processing Implementation
```rust
pub struct Base64Processor {
    max_size: usize,
    allowed_mime_types: HashSet<String>,
}

impl Base64Processor {
    pub fn decode_image_data(&self, data: &str, mime_type: &str) -> Result<Vec<u8>> {
        // Validate base64 format
        self.validate_base64_format(data)?;
        
        // Check size limits before decoding
        self.check_size_limits(data)?;
        
        // Decode base64 data
        let decoded = base64::decode(data)
            .map_err(|e| ProcessingError::InvalidBase64(e.to_string()))?;
        
        // Validate image format matches MIME type
        self.validate_image_format(&decoded, mime_type)?;
        
        Ok(decoded)
    }
    
    pub fn validate_base64_format(&self, data: &str) -> Result<()>;
    pub fn check_size_limits(&self, data: &str) -> Result<()>;
    pub fn validate_image_format(&self, data: &[u8], mime_type: &str) -> Result<()>;
}
```

## Implementation Notes
Add base64 processing comments:
```rust
// ACP requires robust base64 data handling for binary content:
// 1. Image content: base64-encoded image data with MIME type validation
// 2. Audio content: base64-encoded audio data with format validation
// 3. Blob resources: base64-encoded arbitrary binary data
// 4. Size limits: Prevent DoS attacks with large base64 data
// 5. Format validation: Ensure decoded data matches declared MIME type
//
// All base64 processing must include security validation and error handling.
```

### Security and Validation
- [ ] Validate base64 format before decoding to prevent malformed data
- [ ] Check decoded data matches declared MIME type
- [ ] Implement size limits to prevent memory exhaustion
- [ ] Add rate limiting for base64 processing operations
- [ ] Sanitize decoded data to prevent malicious content

### Format-Specific Validation
```rust
pub fn validate_image_format(data: &[u8], mime_type: &str) -> Result<()> {
    match mime_type {
        "image/png" => validate_png_header(data),
        "image/jpeg" => validate_jpeg_header(data),
        "image/gif" => validate_gif_header(data),
        "image/webp" => validate_webp_header(data),
        _ => Err(ProcessingError::UnsupportedImageFormat(mime_type.to_string())),
    }
}

pub fn validate_audio_format(data: &[u8], mime_type: &str) -> Result<()> {
    match mime_type {
        "audio/wav" => validate_wav_header(data),
        "audio/mp3" => validate_mp3_header(data),
        "audio/ogg" => validate_ogg_header(data),
        _ => Err(ProcessingError::UnsupportedAudioFormat(mime_type.to_string())),
    }
}
```

### Error Handling
- [ ] Handle malformed base64 data gracefully
- [ ] Provide clear error messages for base64 validation failures
- [ ] Handle size limit exceeded errors
- [ ] Add format validation error responses
- [ ] Support partial processing recovery for batch operations

### Performance Optimization
- [ ] Optimize base64 decoding for large data
- [ ] Support streaming base64 processing
- [ ] Add memory usage monitoring and limits
- [ ] Implement efficient validation algorithms
- [ ] Cache validation results for repeated data

## Testing Requirements
- [ ] Test base64 encoding/decoding for various data sizes
- [ ] Test image data processing with different formats
- [ ] Test audio data processing with different formats
- [ ] Test blob resource processing with binary data
- [ ] Test malformed base64 data handling
- [ ] Test size limit enforcement and error responses
- [ ] Test format validation against MIME types
- [ ] Test security validation for malicious data

## Configuration Support
- [ ] Add configurable size limits for different content types
- [ ] Support MIME type allowlists for security
- [ ] Configure validation strictness levels
- [ ] Add performance tuning parameters
- [ ] Support format-specific processing options

## Integration Points
- [ ] Connect to content block processing system
- [ ] Integrate with MIME type validation
- [ ] Connect to security and size validation systems
- [ ] Integrate with error handling and response systems

## Acceptance Criteria
- Robust base64 encoding/decoding for all binary content types
- Image data processing with format validation
- Audio data processing with format validation  
- Blob resource processing for embedded resources
- Size limits and security validation for all base64 data
- Format validation ensuring decoded data matches MIME types
- Comprehensive error handling for malformed data
- Performance optimization for large base64 content
- Complete test coverage for all base64 processing scenarios
- Integration with existing content processing systems