use crate::base64_processor::Base64ProcessorError;
use crate::content_block_processor::ContentBlockProcessorError;
use crate::content_security_validator::ContentSecurityError;
use crate::mime_type_validator::MimeTypeValidationError;
use serde_json::{json, Value};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

// ACP content processing requires comprehensive error handling:
// 1. Validation errors: Clear messages for malformed content
// 2. Capability errors: Explain capability requirements
// 3. Size limit errors: Include limit information
// 4. Security errors: Generic messages to avoid information disclosure
// 5. Format errors: Suggest corrective actions
//
// All errors must include structured data for client handling.

/// JSON-RPC 2.0 error structure following ACP specification
#[derive(Debug, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

/// Content processing error with ACP compliance
#[derive(Debug, Error)]
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

    #[error("Memory pressure: insufficient memory for content processing")]
    MemoryPressure,

    #[error("Resource contention: processing queue full")]
    ResourceContention,

    #[error("Invalid URI format: {uri}")]
    InvalidUri { uri: String },

    #[error("Missing required field: {field}")]
    MissingRequiredField { field: String },

    #[error("Content validation failed: {details}")]
    ContentValidationFailed { details: String },

    #[error("Format detection failed: {reason}")]
    FormatDetectionFailed { reason: String },
}

/// Error context for debugging and correlation
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub correlation_id: String,
    pub processing_stage: String,
    pub content_type: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self {
            correlation_id: Uuid::new_v4().to_string(),
            processing_stage: "unknown".to_string(),
            content_type: None,
            metadata: HashMap::new(),
        }
    }
}

/// Convert ContentSecurityError to ACP-compliant JSON-RPC error
pub fn convert_content_security_error_to_acp(
    error: ContentSecurityError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let ctx = context.unwrap_or_default();

    match error {
        ContentSecurityError::SecurityValidationFailed {
            reason,
            policy_violated,
        } => JsonRpcError {
            code: -32602,
            message: "Content security validation failed".to_string(),
            data: Some(json!({
                "error": "security_validation_failed",
                "details": reason,
                "policyViolated": policy_violated,
                "suggestion": "Review content security policies and ensure compliance",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::SuspiciousContentDetected {
            threat_type,
            details,
        } => JsonRpcError {
            code: -32602,
            message: "Suspicious content detected".to_string(),
            data: Some(json!({
                "error": "suspicious_content_detected",
                "threatType": threat_type,
                "details": details,
                "suggestion": "Remove suspicious content or use a lower security level",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::DoSProtectionTriggered {
            protection_type,
            threshold,
        } => JsonRpcError {
            code: -32602,
            message: "DoS protection triggered".to_string(),
            data: Some(json!({
                "error": "dos_protection_triggered",
                "protectionType": protection_type,
                "threshold": threshold,
                "suggestion": "Reduce content size or processing complexity",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::UriSecurityViolation { uri, reason } => JsonRpcError {
            code: -32602,
            message: "URI security violation".to_string(),
            data: Some(json!({
                "error": "uri_security_violation",
                "uri": uri,
                "details": reason,
                "suggestion": "Use allowed URI schemes and avoid private/local addresses",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::Base64SecurityViolation { reason } => JsonRpcError {
            code: -32602,
            message: "Base64 security violation".to_string(),
            data: Some(json!({
                "error": "base64_security_violation",
                "details": reason,
                "suggestion": "Ensure base64 data is valid and within size limits",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::ContentTypeSpoofingDetected { declared, actual } => JsonRpcError {
            code: -32602,
            message: "Content type spoofing detected".to_string(),
            data: Some(json!({
                "error": "content_type_spoofing_detected",
                "declaredType": declared,
                "actualType": actual,
                "suggestion": "Ensure declared MIME type matches actual content format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::ContentSanitizationFailed { reason } => JsonRpcError {
            code: -32602,
            message: "Content sanitization failed".to_string(),
            data: Some(json!({
                "error": "content_sanitization_failed",
                "details": reason,
                "suggestion": "Remove potentially dangerous content patterns",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::SsrfProtectionTriggered { target, reason } => JsonRpcError {
            code: -32602,
            message: "SSRF protection triggered".to_string(),
            data: Some(json!({
                "error": "ssrf_protection_triggered",
                "target": target,
                "details": reason,
                "suggestion": "Avoid accessing private networks or sensitive endpoints",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::ProcessingTimeout { timeout } => JsonRpcError {
            code: -32000,
            message: "Processing timeout".to_string(),
            data: Some(json!({
                "error": "processing_timeout",
                "timeoutMs": timeout,
                "suggestion": "Reduce content complexity or increase timeout limits",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::MemoryLimitExceeded { actual, limit } => JsonRpcError {
            code: -32602,
            message: "Memory limit exceeded".to_string(),
            data: Some(json!({
                "error": "memory_limit_exceeded",
                "actualBytes": actual,
                "limitBytes": limit,
                "suggestion": "Reduce content size or increase memory limits",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::RateLimitExceeded { operation } => JsonRpcError {
            code: -32000,
            message: "Rate limit exceeded".to_string(),
            data: Some(json!({
                "error": "rate_limit_exceeded",
                "operation": operation,
                "suggestion": "Reduce request frequency or wait before retrying",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::ContentArrayTooLarge { length, max_length } => JsonRpcError {
            code: -32602,
            message: "Content array too large".to_string(),
            data: Some(json!({
                "error": "content_array_too_large",
                "arrayLength": length,
                "maxLength": max_length,
                "suggestion": "Reduce the number of content blocks in the array",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::InvalidContentEncoding { encoding } => JsonRpcError {
            code: -32602,
            message: "Invalid content encoding".to_string(),
            data: Some(json!({
                "error": "invalid_content_encoding",
                "encoding": encoding,
                "suggestion": "Use supported content encoding formats",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentSecurityError::MaliciousPatternDetected { pattern_type } => JsonRpcError {
            code: -32602,
            message: "Malicious pattern detected".to_string(),
            data: Some(json!({
                "error": "malicious_pattern_detected",
                "patternType": pattern_type,
                "suggestion": "Remove or sanitize detected malicious patterns",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
    }
}

/// Convert Base64ProcessorError to ACP-compliant JSON-RPC error
pub fn convert_base64_error_to_acp(
    error: Base64ProcessorError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let ctx = context.unwrap_or_default();

    match error {
        Base64ProcessorError::InvalidBase64(details) => JsonRpcError {
            code: -32602,
            message: "Invalid base64 data".to_string(),
            data: Some(json!({
                "error": "invalid_base64_format",
                "details": details,
                "suggestion": "Ensure base64 data is properly encoded with correct padding",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::SizeExceeded { limit, actual } => JsonRpcError {
            code: -32602,
            message: "Content size exceeded maximum limit".to_string(),
            data: Some(json!({
                "error": "content_size_exceeded",
                "providedSize": actual,
                "maxSize": limit,
                "suggestion": "Reduce content size or split into smaller parts",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::MimeTypeNotAllowed(mime_type) => {
            // Get supported MIME types based on context
            let supported_types = get_supported_mime_types(&ctx.content_type);

            JsonRpcError {
                code: -32602,
                message: format!("Unsupported content type: {}", mime_type),
                data: Some(json!({
                    "error": "unsupported_content_type",
                    "contentType": mime_type,
                    "supportedTypes": supported_types,
                    "suggestion": "Use one of the supported content types",
                    "correlationId": ctx.correlation_id,
                    "stage": ctx.processing_stage
                })),
            }
        }
        Base64ProcessorError::FormatMismatch { expected, actual } => JsonRpcError {
            code: -32602,
            message: format!(
                "Content format validation failed: expected {}, got {}",
                expected, actual
            ),
            data: Some(json!({
                "error": "format_mismatch",
                "expectedFormat": expected,
                "actualFormat": actual,
                "suggestion": "Ensure content data matches the declared MIME type",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::UnsupportedImageFormat(format) => JsonRpcError {
            code: -32602,
            message: format!("Unsupported image format: {}", format),
            data: Some(json!({
                "error": "unsupported_image_format",
                "format": format,
                "supportedFormats": ["png", "jpeg", "gif", "webp"],
                "suggestion": "Convert image to a supported format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::UnsupportedAudioFormat(format) => JsonRpcError {
            code: -32602,
            message: format!("Unsupported audio format: {}", format),
            data: Some(json!({
                "error": "unsupported_audio_format",
                "format": format,
                "supportedFormats": ["wav", "mp3", "mpeg", "ogg", "aac"],
                "suggestion": "Convert audio to a supported format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::ProcessingTimeout { timeout } => JsonRpcError {
            code: -32603,
            message: "Processing timeout exceeded".to_string(),
            data: Some(json!({
                "error": "processing_timeout",
                "timeoutMs": timeout,
                "suggestion": "Reduce content size or complexity",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::MemoryAllocationFailed => JsonRpcError {
            code: -32603,
            message: "Insufficient memory for processing".to_string(),
            data: Some(json!({
                "error": "memory_allocation_failed",
                "suggestion": "Reduce content size or retry later",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::CapabilityNotSupported { capability } => JsonRpcError {
            code: -32602,
            message: format!("Capability not supported: {}", capability),
            data: Some(json!({
                "error": "capability_not_supported",
                "requiredCapability": capability,
                "suggestion": "Check agent capabilities before sending content",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::SecurityValidationFailed => JsonRpcError {
            code: -32602,
            message: "Security validation failed".to_string(),
            data: Some(json!({
                "error": "security_validation_failed",
                "suggestion": "Content failed security validation checks",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::ContentValidationFailed { details } => JsonRpcError {
            code: -32602,
            message: "Content validation failed".to_string(),
            data: Some(json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        Base64ProcessorError::EnhancedSecurityValidationFailed(security_error) => {
            convert_content_security_error_to_acp(security_error, Some(ctx))
        }
        Base64ProcessorError::MimeTypeValidationFailed(mime_error) => {
            convert_mime_type_error_to_acp(mime_error, Some(ctx))
        }
    }
}

/// Convert MimeTypeValidationError to ACP-compliant JSON-RPC error
pub fn convert_mime_type_error_to_acp(
    error: MimeTypeValidationError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let ctx = context.unwrap_or_default();

    match error {
        MimeTypeValidationError::UnsupportedMimeType {
            content_type,
            mime_type,
            allowed_types,
            suggestion,
        } => JsonRpcError {
            code: -32602,
            message: format!("Unsupported MIME type for {}: {}", content_type, mime_type),
            data: Some(json!({
                "error": "unsupported_mime_type",
                "contentType": content_type,
                "providedMimeType": mime_type,
                "allowedTypes": allowed_types,
                "suggestion": suggestion.unwrap_or_else(|| format!("Use one of the supported {} MIME types", content_type)),
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        MimeTypeValidationError::SecurityBlocked {
            mime_type,
            reason,
            allowed_categories,
        } => JsonRpcError {
            code: -32602,
            message: format!("MIME type blocked for security reasons: {}", mime_type),
            data: Some(json!({
                "error": "mime_type_security_blocked",
                "providedMimeType": mime_type,
                "reason": reason,
                "allowedCategories": allowed_categories,
                "suggestion": "Use a MIME type from allowed categories",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        MimeTypeValidationError::FormatMismatch {
            expected,
            detected,
            mime_type,
        } => JsonRpcError {
            code: -32602,
            message: format!(
                "MIME type format validation failed: expected {}, detected {}",
                expected, detected
            ),
            data: Some(json!({
                "error": "mime_type_format_mismatch",
                "declaredMimeType": mime_type,
                "expectedFormat": expected,
                "detectedFormat": detected,
                "suggestion": "Ensure content data matches the declared MIME type",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        MimeTypeValidationError::InvalidFormat { mime_type } => JsonRpcError {
            code: -32602,
            message: format!("Invalid MIME type format: {}", mime_type),
            data: Some(json!({
                "error": "invalid_mime_type_format",
                "providedMimeType": mime_type,
                "suggestion": "Provide a valid MIME type in format 'type/subtype'",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        MimeTypeValidationError::ContentValidation { details } => JsonRpcError {
            code: -32602,
            message: "Content validation failed".to_string(),
            data: Some(json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
    }
}

/// Convert ContentBlockProcessorError to ACP-compliant JSON-RPC error
pub fn convert_content_block_error_to_acp(
    error: ContentBlockProcessorError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let ctx = context.unwrap_or_default();

    match error {
        ContentBlockProcessorError::Base64Error(base64_error) => {
            convert_base64_error_to_acp(base64_error, Some(ctx))
        }
        ContentBlockProcessorError::UnsupportedContentType(content_type) => JsonRpcError {
            code: -32602,
            message: format!("Unsupported content type: {}", content_type),
            data: Some(json!({
                "error": "unsupported_content_type",
                "contentType": content_type,
                "supportedTypes": ["text", "image", "audio", "resource", "resource_link"],
                "suggestion": "Use one of the supported content block types",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::MissingRequiredField(field) => JsonRpcError {
            code: -32602,
            message: format!("Missing required field: {}", field),
            data: Some(json!({
                "error": "missing_required_field",
                "field": field,
                "suggestion": "Ensure all required fields are present in content block",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::InvalidUri(uri) => JsonRpcError {
            code: -32602,
            message: "Invalid URI format".to_string(),
            data: Some(json!({
                "error": "invalid_uri",
                "uri": uri,
                "suggestion": "Provide a valid URI with proper scheme (http, https, file, etc.)",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ContentSizeExceeded { actual, limit } => JsonRpcError {
            code: -32602,
            message: "Content size exceeded maximum limit".to_string(),
            data: Some(json!({
                "error": "content_size_exceeded",
                "providedSize": actual,
                "maxSize": limit,
                "suggestion": "Reduce content size or split into smaller parts",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ResourceValidation(details) => JsonRpcError {
            code: -32602,
            message: "Resource validation failed".to_string(),
            data: Some(json!({
                "error": "resource_validation_failed",
                "details": details,
                "suggestion": "Check resource structure and content format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ResourceLinkValidation(details) => JsonRpcError {
            code: -32602,
            message: "Resource link validation failed".to_string(),
            data: Some(json!({
                "error": "resource_link_validation_failed",
                "details": details,
                "suggestion": "Verify resource link URI and metadata",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::InvalidAnnotation(details) => JsonRpcError {
            code: -32602,
            message: "Invalid annotation".to_string(),
            data: Some(json!({
                "error": "invalid_annotation",
                "details": details,
                "suggestion": "Check annotation format and structure",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ProcessingTimeout { timeout } => JsonRpcError {
            code: -32603,
            message: "Processing timeout exceeded".to_string(),
            data: Some(json!({
                "error": "processing_timeout",
                "timeoutMs": timeout,
                "suggestion": "Reduce content size or complexity",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::CapabilityNotSupported { capability } => JsonRpcError {
            code: -32602,
            message: format!("Capability not supported: {}", capability),
            data: Some(json!({
                "error": "capability_not_supported",
                "requiredCapability": capability,
                "suggestion": "Check agent capabilities before sending content",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ContentValidationFailed { details } => JsonRpcError {
            code: -32602,
            message: "Content validation failed".to_string(),
            data: Some(json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::InvalidContentStructure { details } => JsonRpcError {
            code: -32602,
            message: "Invalid content structure".to_string(),
            data: Some(json!({
                "error": "invalid_content_structure",
                "details": details,
                "suggestion": "Verify content block follows ACP specification",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::MemoryAllocationFailed => JsonRpcError {
            code: -32603,
            message: "Memory allocation failed during processing".to_string(),
            data: Some(json!({
                "error": "memory_allocation_failed",
                "suggestion": "Reduce content size or retry later",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::PartialBatchFailure { successful, total } => JsonRpcError {
            code: -32603,
            message: format!(
                "Batch processing partially failed: {}/{} items processed",
                successful, total
            ),
            data: Some(json!({
                "error": "partial_batch_failure",
                "successfulItems": successful,
                "totalItems": total,
                "suggestion": "Review individual item errors for details",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ResourceLinkFetchFailed { uri } => JsonRpcError {
            code: -32603,
            message: "Resource link fetch failed".to_string(),
            data: Some(json!({
                "error": "resource_link_fetch_failed",
                "uri": uri,
                "suggestion": "Verify resource link is accessible",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ContentArrayValidationFailed { details } => JsonRpcError {
            code: -32602,
            message: "Content array validation failed".to_string(),
            data: Some(json!({
                "error": "content_array_validation_failed",
                "details": details,
                "suggestion": "Check content array structure and elements",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentBlockProcessorError::ContentSecurityValidationFailed(security_error) => {
            convert_content_security_error_to_acp(security_error.clone(), Some(ctx))
        }
    }
}

/// Convert enhanced ContentProcessingError to ACP-compliant JSON-RPC error
pub fn convert_content_processing_error_to_acp(
    error: ContentProcessingError,
    context: Option<ErrorContext>,
) -> JsonRpcError {
    let ctx = context.unwrap_or_default();

    match error {
        ContentProcessingError::InvalidStructure(details) => JsonRpcError {
            code: -32602,
            message: "Invalid content block structure".to_string(),
            data: Some(json!({
                "error": "invalid_structure",
                "details": details,
                "suggestion": "Verify content block follows ACP specification",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::UnsupportedContentType {
            content_type,
            supported,
        } => JsonRpcError {
            code: -32602,
            message: format!("Unsupported content type: {}", content_type),
            data: Some(json!({
                "error": "unsupported_content_type",
                "contentType": content_type,
                "supportedTypes": supported,
                "suggestion": "Use one of the supported content types",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::InvalidBase64(details) => JsonRpcError {
            code: -32602,
            message: "Invalid base64 data".to_string(),
            data: Some(json!({
                "error": "invalid_base64_format",
                "details": details,
                "suggestion": "Ensure base64 data is properly encoded with correct padding",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::ContentSizeExceeded { size, limit } => JsonRpcError {
            code: -32602,
            message: "Content size exceeded maximum limit".to_string(),
            data: Some(json!({
                "error": "content_size_exceeded",
                "providedSize": size,
                "maxSize": limit,
                "suggestion": "Reduce content size or split into smaller parts",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::MimeTypeMismatch { mime_type } => JsonRpcError {
            code: -32602,
            message: format!("MIME type validation failed: {}", mime_type),
            data: Some(json!({
                "error": "mime_type_mismatch",
                "mimeType": mime_type,
                "suggestion": "Ensure content data matches the declared MIME type",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::CapabilityNotSupported { capability } => JsonRpcError {
            code: -32602,
            message: format!(
                "Content type not supported: agent does not support {}",
                capability
            ),
            data: Some(json!({
                "error": "capability_not_supported",
                "requiredCapability": capability,
                "declaredValue": false,
                "suggestion": "Check agent capabilities before sending content",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::SecurityViolation { reason: _ } => JsonRpcError {
            code: -32602,
            message: "Security validation failed".to_string(),
            data: Some(json!({
                "error": "security_violation",
                "suggestion": "Content failed security validation checks",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
                // Note: Not including 'reason' to avoid information disclosure
            })),
        },
        ContentProcessingError::ProcessingTimeout { timeout } => JsonRpcError {
            code: -32603,
            message: "Processing timeout exceeded".to_string(),
            data: Some(json!({
                "error": "processing_timeout",
                "timeoutSeconds": timeout,
                "suggestion": "Reduce content size or complexity",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::MemoryPressure => JsonRpcError {
            code: -32603,
            message: "Insufficient memory for content processing".to_string(),
            data: Some(json!({
                "error": "memory_pressure",
                "suggestion": "Reduce content size or retry later",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::ResourceContention => JsonRpcError {
            code: -32603,
            message: "Processing resources currently unavailable".to_string(),
            data: Some(json!({
                "error": "resource_contention",
                "suggestion": "Retry request after a brief delay",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::InvalidUri { uri } => JsonRpcError {
            code: -32602,
            message: "Invalid URI format".to_string(),
            data: Some(json!({
                "error": "invalid_uri",
                "uri": uri,
                "suggestion": "Provide a valid URI with proper scheme",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::MissingRequiredField { field } => JsonRpcError {
            code: -32602,
            message: format!("Missing required field: {}", field),
            data: Some(json!({
                "error": "missing_required_field",
                "field": field,
                "suggestion": "Ensure all required fields are present",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::ContentValidationFailed { details } => JsonRpcError {
            code: -32602,
            message: "Content validation failed".to_string(),
            data: Some(json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
        ContentProcessingError::FormatDetectionFailed { reason } => JsonRpcError {
            code: -32602,
            message: "Content format detection failed".to_string(),
            data: Some(json!({
                "error": "format_detection_failed",
                "reason": reason,
                "suggestion": "Ensure content format matches MIME type declaration",
                "correlationId": ctx.correlation_id,
                "stage": ctx.processing_stage
            })),
        },
    }
}

/// Get supported MIME types based on content type context
fn get_supported_mime_types(content_type: &Option<String>) -> Vec<String> {
    match content_type.as_deref() {
        Some("image") => vec![
            "image/png".to_string(),
            "image/jpeg".to_string(),
            "image/gif".to_string(),
            "image/webp".to_string(),
        ],
        Some("audio") => vec![
            "audio/wav".to_string(),
            "audio/mp3".to_string(),
            "audio/mpeg".to_string(),
            "audio/ogg".to_string(),
            "audio/aac".to_string(),
        ],
        _ => vec![
            "image/png".to_string(),
            "image/jpeg".to_string(),
            "image/gif".to_string(),
            "image/webp".to_string(),
            "audio/wav".to_string(),
            "audio/mp3".to_string(),
            "audio/mpeg".to_string(),
            "audio/ogg".to_string(),
            "audio/aac".to_string(),
            "application/pdf".to_string(),
            "text/plain".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_error_conversion() {
        let error = Base64ProcessorError::InvalidBase64("Invalid padding".to_string());
        let context = ErrorContext {
            correlation_id: "test-123".to_string(),
            processing_stage: "validation".to_string(),
            content_type: Some("image".to_string()),
            metadata: HashMap::new(),
        };

        let json_rpc_error = convert_base64_error_to_acp(error, Some(context));

        assert_eq!(json_rpc_error.code, -32602);
        assert_eq!(json_rpc_error.message, "Invalid base64 data");

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "invalid_base64_format");
            assert_eq!(data["correlationId"], "test-123");
            assert_eq!(data["stage"], "validation");
            assert!(data["suggestion"].as_str().unwrap().contains("base64"));
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_size_exceeded_error_conversion() {
        let error = Base64ProcessorError::SizeExceeded {
            limit: 1024,
            actual: 2048,
        };

        let context = ErrorContext {
            correlation_id: "test-456".to_string(),
            processing_stage: "size_check".to_string(),
            content_type: None,
            metadata: HashMap::new(),
        };

        let json_rpc_error = convert_base64_error_to_acp(error, Some(context));

        assert_eq!(json_rpc_error.code, -32602);
        assert_eq!(
            json_rpc_error.message,
            "Content size exceeded maximum limit"
        );

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "content_size_exceeded");
            assert_eq!(data["providedSize"], 2048);
            assert_eq!(data["maxSize"], 1024);
            assert_eq!(data["correlationId"], "test-456");
            assert_eq!(data["stage"], "size_check");
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_content_processing_error_conversion() {
        let error = ContentProcessingError::CapabilityNotSupported {
            capability: "audio".to_string(),
        };

        let json_rpc_error = convert_content_processing_error_to_acp(error, None);

        assert_eq!(json_rpc_error.code, -32602);
        assert!(json_rpc_error.message.contains("audio"));

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "capability_not_supported");
            assert_eq!(data["requiredCapability"], "audio");
            assert_eq!(data["declaredValue"], false);
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_security_violation_no_info_disclosure() {
        let error = ContentProcessingError::SecurityViolation {
            reason: "sensitive internal details".to_string(),
        };

        let json_rpc_error = convert_content_processing_error_to_acp(error, None);

        assert_eq!(json_rpc_error.code, -32602);
        assert_eq!(json_rpc_error.message, "Security validation failed");

        if let Some(data) = json_rpc_error.data {
            assert_eq!(data["error"], "security_violation");
            // Ensure sensitive reason is not included
            assert!(data.get("reason").is_none());
        } else {
            panic!("Expected error data to be present");
        }
    }

    #[test]
    fn test_default_error_context() {
        let context = ErrorContext::default();

        assert!(!context.correlation_id.is_empty());
        assert_eq!(context.processing_stage, "unknown");
        assert!(context.content_type.is_none());
        assert!(context.metadata.is_empty());
    }

    #[test]
    fn test_get_supported_mime_types() {
        let image_types = get_supported_mime_types(&Some("image".to_string()));
        assert!(image_types.contains(&"image/png".to_string()));
        assert!(image_types.contains(&"image/jpeg".to_string()));
        assert!(!image_types.contains(&"audio/wav".to_string()));

        let audio_types = get_supported_mime_types(&Some("audio".to_string()));
        assert!(audio_types.contains(&"audio/wav".to_string()));
        assert!(audio_types.contains(&"audio/mp3".to_string()));
        assert!(!audio_types.contains(&"image/png".to_string()));

        let all_types = get_supported_mime_types(&None);
        assert!(all_types.contains(&"image/png".to_string()));
        assert!(all_types.contains(&"audio/wav".to_string()));
        assert!(all_types.contains(&"application/pdf".to_string()));
    }
}
