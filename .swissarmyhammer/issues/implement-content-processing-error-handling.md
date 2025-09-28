# Implement Content Processing Error Handling

## Problem
Our content processing lacks comprehensive error handling for the various failure scenarios that can occur during content validation, processing, and conversion. We need robust error handling with proper ACP error codes and recovery mechanisms.

## Error Scenarios to Handle
Based on ACP content specification:

**Content Validation Errors:**
- Invalid base64 data in image/audio content
- Unsupported MIME types for content blocks
- Missing required fields in content blocks
- Content that exceeds capability restrictions
- Malformed content structure

**Processing Errors:**
- Base64 decoding failures
- Image/audio format validation failures
- URI parsing and validation errors
- Content size limit exceeded
- Security validation failures

**System Errors:**
- Memory exhaustion during content processing
- Processing timeouts for large content
- Network errors for resource links
- File system errors for embedded resources

## Implementation Tasks

### Content Validation Error Handling
- [ ] Handle missing required fields in content blocks
- [ ] Validate content block structure and format
- [ ] Return proper errors for capability restriction violations
- [ ] Handle unknown or unsupported content types
- [ ] Validate content arrays and nested structures

### Base64 Processing Error Handling
- [ ] Handle invalid base64 format errors
- [ ] Catch base64 decoding exceptions
- [ ] Handle base64 padding and encoding issues
- [ ] Return clear errors for malformed base64 data
- [ ] Handle partial base64 data corruption

### MIME Type and Format Error Handling
- [ ] Handle unsupported MIME types with clear messages
- [ ] Return available MIME types in error responses
- [ ] Handle MIME type format validation errors
- [ ] Catch format detection failures
- [ ] Handle MIME type security violations

### Size and Resource Error Handling
- [ ] Handle content size limit exceeded errors
- [ ] Catch memory allocation failures during processing
- [ ] Handle processing timeout errors
- [ ] Return resource usage information in errors
- [ ] Handle concurrent processing limit exceeded

## Error Response Implementation
```rust
#[derive(Debug, thiserror::Error)]
pub enum ContentProcessingError {
    #[error("Invalid content block structure: {0}")]
    InvalidStructure(String),
    
    #[error("Unsupported content type: {content_type}, supported types: {supported:?}")]
    UnsupportedContentType {
        content_type: String,
        supported: Vec<String>,
    },
    
    #[error("Invalid base64 data: {0}")]
    InvalidBase64(String),
    
    #[error("Content size exceeded: {size} > {limit}")]
    ContentSizeExceeded { size: usize, limit: usize },
    
    #[error("MIME type validation failed: {mime_type} does not match content format")]
    MimeTypeMismatch { mime_type: String },
    
    #[error("Content capability not supported: {capability}")]
    CapabilityNotSupported { capability: String },
    
    #[error("Security validation failed: {reason}")]
    SecurityViolation { reason: String },
    
    #[error("Processing timeout: content processing exceeded {timeout}s")]
    ProcessingTimeout { timeout: u64 },
}
```

## Implementation Notes
Add content error handling comments:
```rust
// ACP content processing requires comprehensive error handling:
// 1. Validation errors: Clear messages for malformed content
// 2. Capability errors: Explain capability requirements
// 3. Size limit errors: Include limit information
// 4. Security errors: Generic messages to avoid information disclosure
// 5. Format errors: Suggest corrective actions
//
// All errors must include structured data for client handling.
```

### ACP Error Response Format
```rust
pub fn convert_to_acp_error(error: ContentProcessingError) -> JsonRpcError {
    match error {
        ContentProcessingError::UnsupportedContentType { content_type, supported } => {
            JsonRpcError {
                code: -32602,
                message: format!("Unsupported content type: {}", content_type),
                data: Some(json!({
                    "contentType": content_type,
                    "supportedTypes": supported,
                    "suggestion": "Use one of the supported content types"
                })),
            }
        }
        ContentProcessingError::InvalidBase64(details) => {
            JsonRpcError {
                code: -32602,
                message: "Invalid base64 data".to_string(),
                data: Some(json!({
                    "error": "invalid_base64_format",
                    "details": details,
                    "suggestion": "Ensure base64 data is properly encoded"
                })),
            }
        }
        ContentProcessingError::ContentSizeExceeded { size, limit } => {
            JsonRpcError {
                code: -32602,
                message: "Content size exceeded maximum limit".to_string(),
                data: Some(json!({
                    "providedSize": size,
                    "maxSize": limit,
                    "suggestion": "Reduce content size or split into smaller parts"
                })),
            }
        }
        // ... other error conversions
    }
}
```

### Error Recovery and Graceful Degradation
- [ ] Implement partial content processing for batch operations
- [ ] Support content processing retry with exponential backoff
- [ ] Handle graceful degradation when optional content fails
- [ ] Implement content processing circuit breakers
- [ ] Support fallback processing methods

### Error Context and Debugging
- [ ] Include content processing context in errors
- [ ] Add correlation IDs for error tracking
- [ ] Include processing stage information
- [ ] Add detailed diagnostic information for debugging
- [ ] Support error aggregation for batch processing

### Performance-Related Error Handling
- [ ] Handle memory pressure during content processing
- [ ] Implement processing timeouts with clean cancellation
- [ ] Handle resource contention errors
- [ ] Add backpressure handling for high-volume content
- [ ] Support processing queue overflow handling

## Error Response Examples
For invalid base64 data:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid base64 data in image content",
    "data": {
      "error": "invalid_base64_format",
      "contentType": "image",
      "position": 1247,
      "suggestion": "Check base64 encoding and ensure proper padding"
    }
  }
}
```

For capability restriction violation:
```json
{
  "error": {
    "code": -32602,
    "message": "Content type not supported: agent does not support audio content",
    "data": {
      "contentType": "audio",
      "requiredCapability": "promptCapabilities.audio",
      "declaredValue": false,
      "supportedTypes": ["text", "image", "resource_link"]
    }
  }
}
```

For content size exceeded:
```json
{
  "error": {
    "code": -32602,
    "message": "Content size exceeded maximum limit",
    "data": {
      "contentType": "image",
      "providedSize": 52428800,
      "maxSize": 10485760,
      "suggestion": "Compress image or reduce resolution"
    }
  }
}
```

## Testing Requirements
- [ ] Test all content validation error scenarios
- [ ] Test base64 processing error handling
- [ ] Test MIME type validation error responses
- [ ] Test size limit enforcement and error messages
- [ ] Test security validation error handling
- [ ] Test error response format compliance
- [ ] Test error recovery and graceful degradation
- [ ] Test concurrent error handling

## Integration Points
- [ ] Connect to existing content processing system
- [ ] Integrate with logging and monitoring systems
- [ ] Connect to error reporting and alerting
- [ ] Integrate with client error handling expectations

## Acceptance Criteria
- Comprehensive error handling for all content processing failure modes
- Proper ACP error codes and structured error responses
- Clear, actionable error messages with suggestions
- Error recovery and graceful degradation where possible
- Performance error handling with proper timeouts
- Security-aware error messages that don't leak information
- Complete test coverage for all error scenarios
- Integration with existing error handling systems
- Proper error correlation and debugging support