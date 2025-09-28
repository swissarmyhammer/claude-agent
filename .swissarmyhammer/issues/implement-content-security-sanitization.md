# Implement Content Security and Sanitization

## Problem
Our content processing lacks comprehensive security measures to handle potentially malicious or problematic content blocks. We need security validation, sanitization, and protection against various attack vectors through content.

## Security Threats and Requirements
Based on ACP content specification and security best practices:

**Security Threats:**
- **Malformed Base64 Data**: Could cause DoS through memory exhaustion
- **Malicious URIs**: Could reference dangerous resources or perform SSRF attacks
- **Content Type Spoofing**: MIME type doesn't match actual content
- **Code Injection**: Malicious content in embedded resources
- **Size-based DoS**: Extremely large content to exhaust resources

**Security Requirements:**
- Base64 data validation and size limits
- URI validation and security filtering
- Content type validation and spoofing detection
- Size limits and resource exhaustion protection
- Content sanitization for potentially dangerous data

## Implementation Tasks

### Base64 Data Security
- [ ] Validate base64 format before decoding to prevent malformed data attacks
- [ ] Implement size limits for base64 data to prevent memory exhaustion
- [ ] Add rate limiting for base64 processing operations
- [ ] Validate decoded data integrity and format consistency
- [ ] Detect and prevent base64 data designed to exploit decoders

### URI Security and Validation
- [ ] Implement comprehensive URI format validation
- [ ] Block dangerous URI schemes (file://, javascript:, data:, etc.)
- [ ] Validate file URIs are within allowed boundaries
- [ ] Prevent server-side request forgery (SSRF) through URI validation
- [ ] Add URI length limits and format sanitization

### Content Size Limits and DoS Protection
- [ ] Implement configurable size limits for all content types
- [ ] Add memory usage monitoring during content processing
- [ ] Set limits on total content size per request
- [ ] Implement processing timeouts for content validation
- [ ] Add rate limiting for content processing operations

### Content Type Validation and Spoofing Detection
- [ ] Validate actual content format matches declared MIME type
- [ ] Detect content type spoofing attempts
- [ ] Implement magic number validation for binary formats
- [ ] Add content sniffing to verify format claims
- [ ] Block content types that don't match expected patterns

## Security Implementation
```rust
pub struct ContentSecurityValidator {
    max_base64_size: usize,
    max_total_content_size: usize,
    allowed_uri_schemes: HashSet<String>,
    blocked_uri_patterns: Vec<Regex>,
    require_format_validation: bool,
    enable_content_sniffing: bool,
}

impl ContentSecurityValidator {
    pub fn validate_content_security(&self, content: &ContentBlock) -> Result<()> {
        match content {
            ContentBlock::Image(img) => self.validate_image_security(img),
            ContentBlock::Audio(audio) => self.validate_audio_security(audio),
            ContentBlock::Resource(resource) => self.validate_resource_security(resource),
            ContentBlock::ResourceLink(link) => self.validate_resource_link_security(link),
            ContentBlock::Text(text) => self.validate_text_security(text),
        }
    }
    
    fn validate_base64_security(&self, data: &str) -> Result<()> {
        // Check size before decoding
        if data.len() > self.max_base64_size {
            return Err(SecurityError::ContentTooLarge);
        }
        
        // Validate base64 format to prevent malformed data
        self.validate_base64_format(data)?;
        
        // Additional security checks
        self.check_base64_patterns(data)?;
        
        Ok(())
    }
}
```

## Implementation Notes
Add content security comments:
```rust
// Content security is critical for ACP compliance and safety:
// 1. Base64 validation: Prevent DoS through malformed or oversized data
// 2. URI security: Block dangerous schemes and SSRF attempts
// 3. Size limits: Prevent resource exhaustion attacks
// 4. Format validation: Detect content type spoofing
// 5. Content sanitization: Remove potentially dangerous elements
//
// All content must pass security validation before processing.
```

### URI Security Implementation
```rust
fn validate_uri_security(&self, uri: &str) -> Result<()> {
    // Parse and validate URI format
    let parsed_uri = Uri::from_str(uri)
        .map_err(|_| SecurityError::InvalidUri(uri.to_string()))?;
    
    // Check URI scheme against allowlist
    if let Some(scheme) = parsed_uri.scheme() {
        if !self.allowed_uri_schemes.contains(scheme.as_str()) {
            return Err(SecurityError::DisallowedUriScheme(scheme.to_string()));
        }
    }
    
    // Check against blocked patterns
    for pattern in &self.blocked_uri_patterns {
        if pattern.is_match(uri) {
            return Err(SecurityError::BlockedUriPattern);
        }
    }
    
    // Validate file URI boundaries (prevent path traversal)
    if uri.starts_with("file://") {
        self.validate_file_uri_boundaries(uri)?;
    }
    
    Ok(())
}
```

### Content Sanitization
- [ ] Sanitize text content to remove potentially dangerous elements
- [ ] Validate embedded resource content for code injection
- [ ] Remove or escape dangerous characters in text content
- [ ] Validate structured data (JSON, XML) for malicious payloads
- [ ] Implement content filtering based on detected patterns

### Security Configuration
- [ ] Add configurable security levels (strict, moderate, permissive)
- [ ] Support custom security policies per content type
- [ ] Configure size limits based on system resources
- [ ] Add security logging and monitoring
- [ ] Support security policy updates without restart

### Error Handling and Logging
- [ ] Log security violations for monitoring
- [ ] Return generic error messages to avoid information disclosure
- [ ] Add security event correlation and alerting
- [ ] Implement rate limiting for security violations
- [ ] Support security incident response procedures

## Security Error Responses
```json
{
  "error": {
    "code": -32602,
    "message": "Content rejected for security reasons",
    "data": {
      "reason": "content_too_large",
      "maxSize": 1048576,
      "providedSize": 5242880
    }
  }
}
```

```json
{
  "error": {
    "code": -32602,
    "message": "URI blocked for security reasons",
    "data": {
      "reason": "disallowed_scheme",
      "allowedSchemes": ["http", "https", "file"]
    }
  }
}
```

## Testing Requirements
- [ ] Test base64 data security validation with malformed data
- [ ] Test URI security with dangerous schemes and patterns
- [ ] Test size limits and DoS protection
- [ ] Test content type spoofing detection
- [ ] Test content sanitization effectiveness
- [ ] Test security configuration and policy enforcement
- [ ] Test security logging and monitoring
- [ ] Test performance impact of security validation

## Compliance and Standards
- [ ] Align with OWASP security guidelines for content handling
- [ ] Follow industry best practices for input validation
- [ ] Implement defense in depth security principles
- [ ] Support security audit requirements
- [ ] Add compliance reporting capabilities

## Integration Points
- [ ] Connect to existing content validation system
- [ ] Integrate with logging and monitoring systems
- [ ] Connect to rate limiting and throttling systems
- [ ] Integrate with security incident response systems

## Acceptance Criteria
- Comprehensive security validation for all content types
- Base64 data validation and size limit enforcement
- URI security validation and dangerous scheme blocking
- Content type spoofing detection and prevention
- Size limits and DoS protection for all content
- Content sanitization for potentially dangerous data
- Configurable security policies and levels
- Security logging and monitoring integration
- Complete test coverage for all security scenarios
- Performance optimization minimizing security overhead