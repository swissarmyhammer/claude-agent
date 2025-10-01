use crate::base64_validation;
use crate::content_security_validator::{ContentSecurityError, ContentSecurityValidator};
use crate::mime_type_validator::{MimeTypeValidationError, MimeTypeValidator};
use base64::{engine::general_purpose, Engine as _};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum Base64ProcessorError {
    #[error("Invalid base64 format: {0}")]
    InvalidBase64(String),
    #[error("Data exceeds maximum size limit of {limit} bytes (actual: {actual})")]
    SizeExceeded { limit: usize, actual: usize },
    #[error("Unsupported image format: {0}")]
    UnsupportedImageFormat(String),
    #[error("Unsupported audio format: {0}")]
    UnsupportedAudioFormat(String),
    #[error("Format validation failed: expected {expected}, but data appears to be {actual}")]
    FormatMismatch { expected: String, actual: String },
    #[error("MIME type not allowed: {0}")]
    MimeTypeNotAllowed(String),
    #[error("Processing timeout: operation exceeded {timeout}ms")]
    ProcessingTimeout { timeout: u64 },
    #[error("Memory allocation failed: insufficient memory for processing")]
    MemoryAllocationFailed,
    #[error("Capability not supported: {capability}")]
    CapabilityNotSupported { capability: String },
    #[error("Security validation failed")]
    SecurityValidationFailed,
    #[error("Enhanced security validation failed: {0}")]
    EnhancedSecurityValidationFailed(#[from] ContentSecurityError),
    #[error("Content validation failed: {details}")]
    ContentValidationFailed { details: String },
    #[error("MIME type validation failed: {0}")]
    MimeTypeValidationFailed(#[from] MimeTypeValidationError),
}

#[derive(Clone)]
pub struct Base64Processor {
    max_size: usize,
    allowed_blob_mime_types: HashSet<String>,
    processing_timeout: Duration,
    max_memory_usage: usize,
    enable_capability_validation: bool,
    enable_security_validation: bool,
    supported_capabilities: HashSet<String>,
    content_security_validator: Option<ContentSecurityValidator>,
    mime_type_validator: MimeTypeValidator,
}

impl Default for Base64Processor {
    fn default() -> Self {
        let mut allowed_blob_mime_types = HashSet::new();
        // Image types
        allowed_blob_mime_types.insert("image/png".to_string());
        allowed_blob_mime_types.insert("image/jpeg".to_string());
        allowed_blob_mime_types.insert("image/gif".to_string());
        allowed_blob_mime_types.insert("image/webp".to_string());
        // Audio types
        allowed_blob_mime_types.insert("audio/wav".to_string());
        allowed_blob_mime_types.insert("audio/mp3".to_string());
        allowed_blob_mime_types.insert("audio/mpeg".to_string());
        allowed_blob_mime_types.insert("audio/ogg".to_string());
        allowed_blob_mime_types.insert("audio/aac".to_string());
        // Other types
        allowed_blob_mime_types.insert("application/pdf".to_string());
        allowed_blob_mime_types.insert("text/plain".to_string());

        let mut supported_capabilities = HashSet::new();
        supported_capabilities.insert("image".to_string());
        supported_capabilities.insert("audio".to_string());
        supported_capabilities.insert("text".to_string());

        Self {
            max_size: 10 * 1024 * 1024, // 10MB default limit
            allowed_blob_mime_types,
            processing_timeout: Duration::from_secs(30),
            max_memory_usage: 50 * 1024 * 1024, // 50MB memory limit
            enable_capability_validation: true,
            enable_security_validation: true,
            supported_capabilities,
            content_security_validator: None, // Default to no enhanced security validation
            mime_type_validator: MimeTypeValidator::moderate(),
        }
    }
}

impl Base64Processor {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size,
            ..Default::default()
        }
    }

    pub fn new_with_config(
        max_size: usize,
        processing_timeout: Duration,
        max_memory_usage: usize,
        enable_capability_validation: bool,
        enable_security_validation: bool,
        supported_capabilities: HashSet<String>,
    ) -> Self {
        Self {
            max_size,
            processing_timeout,
            max_memory_usage,
            enable_capability_validation,
            enable_security_validation,
            supported_capabilities,
            content_security_validator: None,
            mime_type_validator: MimeTypeValidator::moderate(),
            ..Default::default()
        }
    }

    pub fn with_enhanced_security(
        max_size: usize,
        content_security_validator: ContentSecurityValidator,
    ) -> Self {
        Self {
            max_size,
            content_security_validator: Some(content_security_validator),
            mime_type_validator: MimeTypeValidator::moderate(),
            ..Default::default()
        }
    }

    pub fn with_enhanced_security_config(
        max_size: usize,
        processing_timeout: Duration,
        max_memory_usage: usize,
        enable_capability_validation: bool,
        enable_security_validation: bool,
        supported_capabilities: HashSet<String>,
        content_security_validator: ContentSecurityValidator,
    ) -> Self {
        Self {
            max_size,
            processing_timeout,
            max_memory_usage,
            enable_capability_validation,
            enable_security_validation,
            supported_capabilities,
            content_security_validator: Some(content_security_validator),
            mime_type_validator: MimeTypeValidator::moderate(),
            ..Default::default()
        }
    }

    /// Check if a capability is supported
    fn validate_capability(&self, capability: &str) -> Result<(), Base64ProcessorError> {
        if !self.enable_capability_validation {
            return Ok(());
        }

        if !self.supported_capabilities.contains(capability) {
            return Err(Base64ProcessorError::CapabilityNotSupported {
                capability: capability.to_string(),
            });
        }
        Ok(())
    }

    /// Perform security validation on content
    fn perform_security_validation(&self, data: &[u8]) -> Result<(), Base64ProcessorError> {
        if !self.enable_security_validation {
            return Ok(());
        }

        // Check for potentially malicious patterns (basic security checks)
        if data.len() > self.max_memory_usage {
            return Err(Base64ProcessorError::MemoryAllocationFailed);
        }

        // Check for suspicious patterns in binary data
        if self.contains_suspicious_patterns(data) {
            return Err(Base64ProcessorError::SecurityValidationFailed);
        }

        Ok(())
    }

    /// Check for suspicious patterns in binary data
    fn contains_suspicious_patterns(&self, data: &[u8]) -> bool {
        // Basic heuristic checks for potentially malicious content
        if data.len() < 16 {
            return false;
        }

        // Check for excessive null bytes (possible data corruption or attack)
        let null_count = data.iter().filter(|&&b| b == 0).count();
        if null_count > data.len() / 2 {
            return true;
        }

        // Check for patterns that might indicate embedded executables
        if data.len() >= 2 && data.starts_with(b"MZ") {
            return true; // DOS/Windows executable
        }
        if data.len() >= 4 && data.starts_with(b"\x7fELF") {
            return true; // Linux ELF executable
        }
        if data.len() >= 3 && data.starts_with(b"\xfe\xed\xfa") {
            return true; // Mach-O binary (partial)
        }
        if data.len() >= 4 && data.starts_with(b"\xcf\xfa\xed\xfe") {
            return true; // Mach-O binary
        }

        false
    }

    /// Perform processing with timeout
    fn with_timeout<F, R>(&self, operation: F) -> Result<R, Base64ProcessorError>
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = operation();
        let elapsed = start.elapsed();

        if elapsed > self.processing_timeout {
            return Err(Base64ProcessorError::ProcessingTimeout {
                timeout: elapsed.as_millis() as u64,
            });
        }

        Ok(result)
    }

    pub fn decode_image_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support
        self.validate_capability("image")?;

        // Enhanced security validation if available
        if let Some(ref validator) = self.content_security_validator {
            validator
                .validate_base64_security(data, "image")
                .map_err(|_e| Base64ProcessorError::SecurityValidationFailed)?;
        }

        // Validate base64 format and size limits
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        // Perform base64 decoding with timeout
        let decoded = self.with_timeout(|| {
            general_purpose::STANDARD
                .decode(data)
                .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))
        })??;

        // Use centralized MIME type validator with format validation
        self.mime_type_validator
            .validate_image_mime_type(mime_type, Some(&decoded))?;

        // Security validation
        self.perform_security_validation(&decoded)?;

        Ok(decoded)
    }

    pub fn decode_audio_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support
        self.validate_capability("audio")?;

        // Enhanced security validation if available
        if let Some(ref validator) = self.content_security_validator {
            validator
                .validate_base64_security(data, "audio")
                .map_err(|_e| Base64ProcessorError::SecurityValidationFailed)?;
        }

        // Validate base64 format and size limits
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        // Perform base64 decoding with timeout
        let decoded = self.with_timeout(|| {
            general_purpose::STANDARD
                .decode(data)
                .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))
        })??;

        // Use centralized MIME type validator with format validation
        self.mime_type_validator
            .validate_audio_mime_type(mime_type, Some(&decoded))?;

        // Security validation
        self.perform_security_validation(&decoded)?;

        Ok(decoded)
    }

    pub fn decode_blob_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        // Validate capability support (general capability for blob data)
        let capability = if mime_type.starts_with("image/") {
            "image"
        } else if mime_type.starts_with("audio/") {
            "audio"
        } else {
            "text" // Default for other blob types like PDF, text
        };
        self.validate_capability(capability)?;

        // Enhanced security validation if available
        if let Some(ref validator) = self.content_security_validator {
            validator
                .validate_base64_security(data, "blob")
                .map_err(|_e| Base64ProcessorError::SecurityValidationFailed)?;
        }

        // Validate MIME type and base64 format
        self.validate_mime_type(mime_type, &self.allowed_blob_mime_types)?;
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        // Perform base64 decoding with timeout
        let decoded = self.with_timeout(|| {
            general_purpose::STANDARD
                .decode(data)
                .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))
        })??;

        // Security validation
        self.perform_security_validation(&decoded)?;

        Ok(decoded)
    }

    fn validate_base64_format(&self, data: &str) -> Result<(), Base64ProcessorError> {
        base64_validation::validate_base64_format(data).map_err(|e| match e {
            base64_validation::Base64ValidationError::EmptyData => {
                Base64ProcessorError::InvalidBase64("Empty base64 data".to_string())
            }
            base64_validation::Base64ValidationError::InvalidCharacters => {
                Base64ProcessorError::InvalidBase64("Contains invalid characters".to_string())
            }
            base64_validation::Base64ValidationError::InvalidPadding => {
                Base64ProcessorError::InvalidBase64("Invalid base64 padding".to_string())
            }
        })
    }

    fn check_size_limits(&self, data: &str) -> Result<(), Base64ProcessorError> {
        // Estimate decoded size (base64 encodes 3 bytes as 4 characters)
        let estimated_decoded_size = (data.len() * 3) / 4;

        if estimated_decoded_size > self.max_size {
            return Err(Base64ProcessorError::SizeExceeded {
                limit: self.max_size,
                actual: estimated_decoded_size,
            });
        }

        Ok(())
    }

    fn validate_mime_type(
        &self,
        mime_type: &str,
        allowed_types: &HashSet<String>,
    ) -> Result<(), Base64ProcessorError> {
        if !allowed_types.contains(mime_type) {
            return Err(Base64ProcessorError::MimeTypeNotAllowed(
                mime_type.to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_base64_format() {
        let processor = Base64Processor::default();

        // Valid base64
        assert!(processor.validate_base64_format("SGVsbG8gV29ybGQ=").is_ok());

        // Empty string
        assert!(processor.validate_base64_format("").is_err());

        // Invalid characters
        assert!(processor.validate_base64_format("Hello!").is_err());

        // Invalid padding
        assert!(processor.validate_base64_format("SGVsbG8").is_err());
    }

    #[test]
    fn test_check_size_limits() {
        let processor = Base64Processor::new(100); // 100 bytes limit

        // Small data (should pass)
        assert!(processor.check_size_limits("SGVsbG8=").is_ok()); // "Hello"

        // Large data (should fail)
        let large_data = "A".repeat(200); // Much larger than 100 bytes when decoded
        assert!(processor.check_size_limits(&large_data).is_err());
    }

    #[test]
    fn test_validate_png_format() {
        let validator = MimeTypeValidator::default();

        // Valid PNG header
        let png_header = b"\x89PNG\r\n\x1a\n";
        assert!(validator
            .validate_image_mime_type("image/png", Some(png_header))
            .is_ok());

        // Invalid PNG header
        let invalid_header = b"NOTPNG\x00\x00";
        assert!(validator
            .validate_image_mime_type("image/png", Some(invalid_header))
            .is_err());
    }

    #[test]
    fn test_validate_jpeg_format() {
        let validator = MimeTypeValidator::default();

        // Valid JPEG header (SOI marker)
        let jpeg_header = b"\xFF\xD8\xFF\xE0";

        let result = validator.validate_image_mime_type("image/jpeg", Some(jpeg_header));
        if let Err(e) = result {
            panic!("JPEG validation should have succeeded but got error: {}", e);
        }

        // Invalid JPEG header
        let invalid_header = b"NOTJPEG\x00";
        let result2 = validator.validate_image_mime_type("image/jpeg", Some(invalid_header));
        if result2.is_ok() {
            panic!("Invalid JPEG header should have been rejected");
        }
    }

    #[test]
    fn test_decode_image_data() {
        let processor = Base64Processor::default();

        // This is a 1x1 PNG in base64
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let result = processor.decode_image_data(png_data, "image/png");
        assert!(result.is_ok());

        // Test with wrong MIME type
        let result = processor.decode_image_data(png_data, "image/jpeg");
        assert!(result.is_err());
    }

    #[test]
    fn test_mime_type_validation() {
        let processor = Base64Processor::default();

        // Test allowed blob MIME type (image)
        assert!(processor
            .validate_mime_type("image/png", &processor.allowed_blob_mime_types)
            .is_ok());

        // Test disallowed MIME type
        assert!(processor
            .validate_mime_type("image/bmp", &processor.allowed_blob_mime_types)
            .is_err());
    }
}
