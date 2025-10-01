use crate::base64_processor::{Base64Processor, Base64ProcessorError};
use crate::content_security_validator::{ContentSecurityError, ContentSecurityValidator};
use crate::error::ToJsonRpcError;
use crate::size_validator::{SizeValidationError, SizeValidator};
use agent_client_protocol::{ContentBlock, TextContent};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, error, warn};

/// Configuration struct for enhanced security settings
#[derive(Debug)]
pub struct EnhancedSecurityConfig {
    pub max_resource_size: usize,
    pub enable_uri_validation: bool,
    pub processing_timeout: Duration,
    pub enable_capability_validation: bool,
    pub supported_capabilities: HashMap<String, bool>,
    pub enable_batch_recovery: bool,
    pub content_security_validator: ContentSecurityValidator,
}

#[derive(Debug, Error, Clone)]
pub enum ContentBlockProcessorError {
    #[error("Base64 processing error: {0}")]
    Base64Error(#[from] Base64ProcessorError),
    #[error("Resource validation error: {0}")]
    ResourceValidation(String),
    #[error("ResourceLink validation error: {0}")]
    ResourceLinkValidation(String),
    #[error("Unsupported content type: {0}")]
    UnsupportedContentType(String),
    #[error("Missing required field: {0}")]
    MissingRequiredField(String),
    #[error("Invalid URI format: {0}")]
    InvalidUri(String),
    #[error("Content size exceeds limit: {actual} > {limit} bytes")]
    ContentSizeExceeded { actual: usize, limit: usize },
    #[error("Invalid annotation: {0}")]
    InvalidAnnotation(String),
    #[error("Processing timeout: operation exceeded {timeout}ms")]
    ProcessingTimeout { timeout: u64 },
    #[error("Capability not supported: {capability}")]
    CapabilityNotSupported { capability: String },
    #[error("Content validation failed: {details}")]
    ContentValidationFailed { details: String },
    #[error("Invalid content structure: {details}")]
    InvalidContentStructure { details: String },
    #[error("Memory allocation failed during processing")]
    MemoryAllocationFailed,
    #[error("Batch processing partially failed: {successful}/{total} items processed")]
    PartialBatchFailure { successful: usize, total: usize },
    #[error("Resource link fetch failed: {uri}")]
    ResourceLinkFetchFailed { uri: String },
    #[error("Content array validation failed: {details}")]
    ContentArrayValidationFailed { details: String },
    #[error("Content security validation failed: {0}")]
    ContentSecurityValidationFailed(#[from] ContentSecurityError),
}

impl ToJsonRpcError for ContentBlockProcessorError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            Self::Base64Error(base64_error) => base64_error.to_json_rpc_code(),
            Self::ResourceValidation(_)
            | Self::ResourceLinkValidation(_)
            | Self::UnsupportedContentType(_)
            | Self::MissingRequiredField(_)
            | Self::InvalidUri(_)
            | Self::ContentSizeExceeded { .. }
            | Self::InvalidAnnotation(_)
            | Self::CapabilityNotSupported { .. }
            | Self::ContentValidationFailed { .. }
            | Self::InvalidContentStructure { .. }
            | Self::ContentArrayValidationFailed { .. } => -32602, // Invalid params
            Self::ProcessingTimeout { .. }
            | Self::MemoryAllocationFailed
            | Self::PartialBatchFailure { .. }
            | Self::ResourceLinkFetchFailed { .. }
            | Self::ContentSecurityValidationFailed(_) => -32603, // Internal error
        }
    }

    fn to_error_data(&self) -> Option<Value> {
        let data = match self {
            Self::Base64Error(base64_error) => return base64_error.to_error_data(),
            Self::ResourceValidation(details) => json!({
                "error": "resource_validation_failed",
                "details": details,
                "suggestion": "Check resource structure and content format"
            }),
            Self::ResourceLinkValidation(details) => json!({
                "error": "resource_link_validation_failed",
                "details": details,
                "suggestion": "Verify resource link URI and metadata"
            }),
            Self::UnsupportedContentType(content_type) => json!({
                "error": "unsupported_content_type",
                "contentType": content_type,
                "supportedTypes": ["text", "image", "audio", "resource", "resource_link"],
                "suggestion": "Use one of the supported content block types"
            }),
            Self::MissingRequiredField(field) => json!({
                "error": "missing_required_field",
                "field": field,
                "suggestion": "Ensure all required fields are present in content block"
            }),
            Self::InvalidUri(uri) => json!({
                "error": "invalid_uri",
                "uri": uri,
                "suggestion": "Provide a valid URI with proper scheme (http, https, file, etc.)"
            }),
            Self::ContentSizeExceeded { actual, limit } => json!({
                "error": "content_size_exceeded",
                "providedSize": actual,
                "maxSize": limit,
                "suggestion": "Reduce content size or split into smaller parts"
            }),
            Self::InvalidAnnotation(details) => json!({
                "error": "invalid_annotation",
                "details": details,
                "suggestion": "Check annotation format and structure"
            }),
            Self::ProcessingTimeout { timeout } => json!({
                "error": "processing_timeout",
                "timeoutMs": timeout,
                "suggestion": "Reduce content size or complexity"
            }),
            Self::CapabilityNotSupported { capability } => json!({
                "error": "capability_not_supported",
                "requiredCapability": capability,
                "suggestion": "Check agent capabilities before sending content"
            }),
            Self::ContentValidationFailed { details } => json!({
                "error": "content_validation_failed",
                "details": details,
                "suggestion": "Check content structure and format"
            }),
            Self::InvalidContentStructure { details } => json!({
                "error": "invalid_content_structure",
                "details": details,
                "suggestion": "Verify content block follows ACP specification"
            }),
            Self::MemoryAllocationFailed => json!({
                "error": "memory_allocation_failed",
                "suggestion": "Reduce content size or retry later"
            }),
            Self::PartialBatchFailure { successful, total } => json!({
                "error": "partial_batch_failure",
                "successfulItems": successful,
                "totalItems": total,
                "suggestion": "Review individual item errors for details"
            }),
            Self::ResourceLinkFetchFailed { uri } => json!({
                "error": "resource_link_fetch_failed",
                "uri": uri,
                "suggestion": "Verify resource link is accessible"
            }),
            Self::ContentArrayValidationFailed { details } => json!({
                "error": "content_array_validation_failed",
                "details": details,
                "suggestion": "Check content array structure and elements"
            }),
            Self::ContentSecurityValidationFailed(security_error) => {
                return security_error.to_error_data();
            }
        };
        Some(data)
    }
}

impl From<SizeValidationError> for ContentBlockProcessorError {
    fn from(error: SizeValidationError) -> Self {
        match error {
            SizeValidationError::SizeExceeded {
                actual, limit, ..
            } => ContentBlockProcessorError::ContentSizeExceeded { actual, limit },
        }
    }
}

#[derive(Debug)]
pub struct ProcessedContent {
    pub content_type: ProcessedContentType,
    pub text_representation: String,
    pub binary_data: Option<Vec<u8>>,
    pub metadata: HashMap<String, String>,
    pub size_bytes: usize,
}

#[derive(Debug, Clone)]
pub enum ProcessedContentType {
    Text,
    Image {
        mime_type: String,
    },
    Audio {
        mime_type: String,
    },
    EmbeddedResource {
        uri: Option<String>,
        mime_type: Option<String>,
    },
    ResourceLink {
        uri: String,
    },
}

pub struct ContentBlockProcessor {
    base64_processor: Base64Processor,
    enable_uri_validation: bool,
    processing_timeout: Duration,
    enable_capability_validation: bool,
    supported_capabilities: HashMap<String, bool>,
    enable_batch_recovery: bool,
    content_security_validator: Option<ContentSecurityValidator>,
    size_validator: SizeValidator,
}

impl Default for ContentBlockProcessor {
    fn default() -> Self {
        let mut supported_capabilities = HashMap::new();
        supported_capabilities.insert("text".to_string(), true);
        supported_capabilities.insert("image".to_string(), true);
        supported_capabilities.insert("audio".to_string(), false); // Disabled by default
        supported_capabilities.insert("resource".to_string(), true);
        supported_capabilities.insert("resource_link".to_string(), true);

        let size_validator = SizeValidator::default();

        Self {
            base64_processor: Base64Processor::default(),
            enable_uri_validation: true,
            processing_timeout: Duration::from_secs(30),
            enable_capability_validation: true,
            supported_capabilities,
            enable_batch_recovery: true,
            content_security_validator: None, // Default to no enhanced security validation
            size_validator,
        }
    }
}

impl ContentBlockProcessor {
    pub fn new(
        base64_processor: Base64Processor,
        max_resource_size: usize,
        enable_uri_validation: bool,
    ) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_content_size: max_resource_size,
            ..Default::default()
        });

        Self {
            base64_processor,
            enable_uri_validation,
            size_validator,
            ..Default::default()
        }
    }

    pub fn new_with_config(
        base64_processor: Base64Processor,
        max_resource_size: usize,
        enable_uri_validation: bool,
        processing_timeout: Duration,
        enable_capability_validation: bool,
        supported_capabilities: HashMap<String, bool>,
        enable_batch_recovery: bool,
    ) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_content_size: max_resource_size,
            ..Default::default()
        });

        Self {
            base64_processor,
            enable_uri_validation,
            processing_timeout,
            enable_capability_validation,
            supported_capabilities,
            enable_batch_recovery,
            content_security_validator: None,
            size_validator,
        }
    }

    pub fn with_enhanced_security(
        base64_processor: Base64Processor,
        max_resource_size: usize,
        enable_uri_validation: bool,
        content_security_validator: ContentSecurityValidator,
    ) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_content_size: max_resource_size,
            ..Default::default()
        });

        Self {
            base64_processor,
            enable_uri_validation,
            content_security_validator: Some(content_security_validator),
            size_validator,
            ..Default::default()
        }
    }

    pub fn with_enhanced_security_config(
        base64_processor: Base64Processor,
        config: EnhancedSecurityConfig,
    ) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_content_size: config.max_resource_size,
            ..Default::default()
        });

        Self {
            base64_processor,
            enable_uri_validation: config.enable_uri_validation,
            processing_timeout: config.processing_timeout,
            enable_capability_validation: config.enable_capability_validation,
            supported_capabilities: config.supported_capabilities,
            enable_batch_recovery: config.enable_batch_recovery,
            content_security_validator: Some(config.content_security_validator),
            size_validator,
        }
    }

    /// Validate capability is supported
    pub fn validate_capability(&self, capability: &str) -> Result<(), ContentBlockProcessorError> {
        if !self.enable_capability_validation {
            return Ok(());
        }

        match self.supported_capabilities.get(capability) {
            Some(&true) => Ok(()),
            Some(&false) => Err(ContentBlockProcessorError::CapabilityNotSupported {
                capability: capability.to_string(),
            }),
            None => Err(ContentBlockProcessorError::CapabilityNotSupported {
                capability: capability.to_string(),
            }),
        }
    }

    /// Perform processing with timeout
    fn with_timeout<F, R>(&self, operation: F) -> Result<R, ContentBlockProcessorError>
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = operation();
        let elapsed = start.elapsed();

        if elapsed > self.processing_timeout {
            return Err(ContentBlockProcessorError::ProcessingTimeout {
                timeout: elapsed.as_millis() as u64,
            });
        }

        Ok(result)
    }

    /// Validate content block structure
    pub fn validate_content_block_structure(
        &self,
        content_block: &ContentBlock,
    ) -> Result<(), ContentBlockProcessorError> {
        // Enhanced security validation first if available
        if let Some(ref validator) = self.content_security_validator {
            validator.validate_content_security(content_block)?;
        }

        match content_block {
            ContentBlock::Text(text_content) => {
                if text_content.text.is_empty() {
                    return Err(ContentBlockProcessorError::InvalidContentStructure {
                        details: "Text content cannot be empty".to_string(),
                    });
                }
            }
            ContentBlock::Image(image_content) => {
                if image_content.data.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "data".to_string(),
                    ));
                }
                if image_content.mime_type.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "mime_type".to_string(),
                    ));
                }
            }
            ContentBlock::Audio(audio_content) => {
                self.validate_capability("audio")?;
                if audio_content.data.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "data".to_string(),
                    ));
                }
                if audio_content.mime_type.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "mime_type".to_string(),
                    ));
                }
            }
            ContentBlock::Resource(_resource_content) => {
                self.validate_capability("resource")?;
                // Resource structure validation will be enhanced when Resource processing is fully implemented
                debug!("Resource content structure validation placeholder");
            }
            ContentBlock::ResourceLink(resource_link) => {
                self.validate_capability("resource_link")?;
                if resource_link.uri.is_empty() {
                    return Err(ContentBlockProcessorError::MissingRequiredField(
                        "uri".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Process a ContentBlock and return structured processed content
    ///
    /// ACP requires support for all 5 ContentBlock types:
    /// 1. Text: Always supported (mandatory)
    /// 2. Image: Base64 data + MIME type validation
    /// 3. Audio: Base64 data + MIME type validation  
    /// 4. Resource: Complex nested structure with text/blob variants
    /// 5. ResourceLink: URI-based resource references with metadata
    ///
    /// Content must be validated against declared prompt capabilities.
    pub fn process_content_block(
        &self,
        content_block: &ContentBlock,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        debug!(
            "Processing content block: {:?}",
            std::mem::discriminant(content_block)
        );

        // Validate content block structure
        self.validate_content_block_structure(content_block)?;

        // Process with timeout
        self.with_timeout(|| self.process_content_block_internal(content_block))?
    }

    fn process_content_block_internal(
        &self,
        content_block: &ContentBlock,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        match content_block {
            ContentBlock::Text(text_content) => {
                self.validate_capability("text")?;
                self.process_text_content(text_content)
            }
            ContentBlock::Image(image_content) => {
                self.validate_capability("image")?;
                // Decode and validate image data using existing base64_processor
                let decoded_data = self
                    .base64_processor
                    .decode_image_data(&image_content.data, &image_content.mime_type)?;

                // Check resource size limit
                self.size_validator.validate_content_size(decoded_data.len())?;

                let mut metadata = HashMap::new();
                metadata.insert("mime_type".to_string(), image_content.mime_type.clone());
                metadata.insert("data_size".to_string(), decoded_data.len().to_string());

                if let Some(ref uri) = image_content.uri {
                    if self.enable_uri_validation {
                        self.validate_uri(uri)?;
                    }
                    metadata.insert("source_uri".to_string(), uri.clone());
                }

                let text_representation = format!(
                    "[Image content: {} ({} bytes){}]",
                    image_content.mime_type,
                    decoded_data.len(),
                    if let Some(ref uri) = image_content.uri {
                        format!(" from {}", uri)
                    } else {
                        " (embedded)".to_string()
                    }
                );

                Ok(ProcessedContent {
                    content_type: ProcessedContentType::Image {
                        mime_type: image_content.mime_type.clone(),
                    },
                    text_representation,
                    binary_data: Some(decoded_data),
                    metadata,
                    size_bytes: image_content.data.len(),
                })
            }
            ContentBlock::Audio(audio_content) => {
                // Decode and validate audio data using existing base64_processor
                let decoded_data = self
                    .base64_processor
                    .decode_audio_data(&audio_content.data, &audio_content.mime_type)?;

                // Check resource size limit
                self.size_validator.validate_content_size(decoded_data.len())?;

                let mut metadata = HashMap::new();
                metadata.insert("mime_type".to_string(), audio_content.mime_type.clone());
                metadata.insert("data_size".to_string(), decoded_data.len().to_string());

                let text_representation = format!(
                    "[Audio content: {} ({} bytes)]",
                    audio_content.mime_type,
                    decoded_data.len()
                );

                Ok(ProcessedContent {
                    content_type: ProcessedContentType::Audio {
                        mime_type: audio_content.mime_type.clone(),
                    },
                    text_representation,
                    binary_data: Some(decoded_data),
                    metadata,
                    size_bytes: audio_content.data.len(),
                })
            }
            ContentBlock::Resource(_resource_content) => {
                // Enhanced processing placeholder for embedded resources
                let mut metadata = HashMap::new();
                metadata.insert("content_type".to_string(), "embedded_resource".to_string());

                let text_representation =
                    "[Embedded Resource - enhanced processing available]".to_string();

                Ok(ProcessedContent {
                    content_type: ProcessedContentType::EmbeddedResource {
                        uri: None,
                        mime_type: None,
                    },
                    text_representation,
                    binary_data: None,
                    metadata,
                    size_bytes: 0,
                })
            }
            ContentBlock::ResourceLink(resource_link) => {
                let mut metadata = HashMap::new();

                if self.enable_uri_validation {
                    self.validate_uri(&resource_link.uri)?;
                }

                metadata.insert("uri".to_string(), resource_link.uri.clone());

                // Add any available resource link metadata
                // Note: Using the pattern from existing code which only accesses .uri
                let text_representation = format!("[Resource Link: {}]", resource_link.uri);

                Ok(ProcessedContent {
                    content_type: ProcessedContentType::ResourceLink {
                        uri: resource_link.uri.clone(),
                    },
                    text_representation,
                    binary_data: None,
                    metadata,
                    size_bytes: 0, // ResourceLink doesn't contain actual content data
                })
            }
        }
    }

    fn process_text_content(
        &self,
        text_content: &TextContent,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let metadata = HashMap::new();

        let content_text = text_content.text.clone();
        let size_bytes = content_text.len();

        Ok(ProcessedContent {
            content_type: ProcessedContentType::Text,
            text_representation: content_text,
            binary_data: None,
            metadata,
            size_bytes,
        })
    }

    fn validate_uri(&self, uri: &str) -> Result<(), ContentBlockProcessorError> {
        if uri.is_empty() {
            return Err(ContentBlockProcessorError::InvalidUri(
                "URI cannot be empty".to_string(),
            ));
        }

        // Basic URI validation
        if !uri.contains(':') {
            return Err(ContentBlockProcessorError::InvalidUri(
                "URI must contain a scheme".to_string(),
            ));
        }

        // Allow common schemes
        let allowed_schemes = ["file", "http", "https", "data", "ftp"];
        let scheme = uri.split(':').next().unwrap_or("");

        if !allowed_schemes.contains(&scheme) {
            warn!("Potentially unsupported URI scheme: {}", scheme);
        }

        Ok(())
    }

    /// Get comprehensive content processing summary for all content blocks with enhanced error handling
    pub fn process_content_blocks(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<ContentProcessingSummary, ContentBlockProcessorError> {
        // Enhanced security validation for content arrays if available
        if let Some(ref validator) = self.content_security_validator {
            validator.validate_content_blocks_security(content_blocks)?;
        }

        if self.enable_batch_recovery {
            self.process_content_blocks_with_recovery(content_blocks)
        } else {
            self.process_content_blocks_strict(content_blocks)
        }
    }

    /// Process content blocks with strict error handling (fail on first error)
    fn process_content_blocks_strict(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<ContentProcessingSummary, ContentBlockProcessorError> {
        let mut text_content = String::new();
        let mut has_binary_content = false;
        let mut processed_contents = Vec::new();
        let mut total_size = 0;
        let mut content_type_counts = HashMap::new();

        for (index, content_block) in content_blocks.iter().enumerate() {
            debug!(
                "Processing content block {} of {}",
                index + 1,
                content_blocks.len()
            );

            let processed = self.process_content_block(content_block).map_err(|e| {
                error!("Failed to process content block at index {}: {}", index, e);
                e
            })?;

            // Accumulate text representation
            text_content.push_str(&processed.text_representation);

            // Track binary content
            if processed.binary_data.is_some() {
                has_binary_content = true;
            }

            // Update size and type counts
            total_size += processed.size_bytes;
            let type_key = self.get_content_type_key(&processed.content_type);
            *content_type_counts.entry(type_key.to_string()).or_insert(0) += 1;

            processed_contents.push(processed);
        }

        Ok(ContentProcessingSummary {
            processed_contents,
            combined_text: text_content,
            has_binary_content,
            total_size_bytes: total_size,
            content_type_counts,
        })
    }

    /// Process content blocks with error recovery (partial processing)
    fn process_content_blocks_with_recovery(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<ContentProcessingSummary, ContentBlockProcessorError> {
        let mut text_content = String::new();
        let mut has_binary_content = false;
        let mut processed_contents = Vec::new();
        let mut total_size = 0;
        let mut content_type_counts = HashMap::new();
        let mut successful_count = 0;
        let mut processing_errors = Vec::new();

        for (index, content_block) in content_blocks.iter().enumerate() {
            debug!(
                "Processing content block {} of {} (with recovery)",
                index + 1,
                content_blocks.len()
            );

            match self.process_content_block_with_retry(content_block, 3) {
                Ok(processed) => {
                    successful_count += 1;

                    // Accumulate text representation
                    text_content.push_str(&processed.text_representation);

                    // Track binary content
                    if processed.binary_data.is_some() {
                        has_binary_content = true;
                    }

                    // Update size and type counts
                    total_size += processed.size_bytes;
                    let type_key = self.get_content_type_key(&processed.content_type);
                    *content_type_counts.entry(type_key.to_string()).or_insert(0) += 1;

                    processed_contents.push(processed);
                }
                Err(e) => {
                    error!(
                        "Failed to process content block at index {} after retries: {}",
                        index, e
                    );

                    // Add placeholder for failed content
                    let fallback_content = self.create_fallback_content(index, &e);

                    // Store error for reporting
                    processing_errors.push((index, e));
                    text_content.push_str(&fallback_content.text_representation);
                    processed_contents.push(fallback_content);
                }
            }
        }

        // If too many failures, return batch failure error
        if successful_count == 0 && !processing_errors.is_empty() {
            return Err(processing_errors.into_iter().next().unwrap().1);
        }

        if successful_count < content_blocks.len() {
            warn!(
                "Partial batch processing: {}/{} content blocks processed successfully",
                successful_count,
                content_blocks.len()
            );
        }

        Ok(ContentProcessingSummary {
            processed_contents,
            combined_text: text_content,
            has_binary_content,
            total_size_bytes: total_size,
            content_type_counts,
        })
    }

    /// Process content block with retry logic
    fn process_content_block_with_retry(
        &self,
        content_block: &ContentBlock,
        max_retries: u32,
    ) -> Result<ProcessedContent, ContentBlockProcessorError> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                // Exponential backoff
                let backoff_ms = std::cmp::min(1000 * (2_u64.pow(attempt - 1)), 10000);
                debug!(
                    "Retrying content block processing after {}ms (attempt {})",
                    backoff_ms,
                    attempt + 1
                );
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }

            match self.process_content_block(content_block) {
                Ok(processed) => {
                    if attempt > 0 {
                        debug!(
                            "Content block processing succeeded on attempt {}",
                            attempt + 1
                        );
                    }
                    return Ok(processed);
                }
                Err(e) => {
                    last_error = Some(e);

                    // Don't retry certain non-transient errors
                    if let Some(ref error) = last_error {
                        if self.is_non_retryable_error(error) {
                            debug!("Non-retryable error encountered, not retrying: {}", error);
                            break;
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Check if error should not be retried
    fn is_non_retryable_error(&self, error: &ContentBlockProcessorError) -> bool {
        match error {
            ContentBlockProcessorError::CapabilityNotSupported { .. } => true,
            ContentBlockProcessorError::MissingRequiredField(_) => true,
            ContentBlockProcessorError::InvalidContentStructure { .. } => true,
            ContentBlockProcessorError::UnsupportedContentType(_) => true,
            ContentBlockProcessorError::Base64Error(base64_error) => {
                matches!(
                    base64_error,
                    crate::base64_processor::Base64ProcessorError::MimeTypeNotAllowed(_)
                        | crate::base64_processor::Base64ProcessorError::CapabilityNotSupported { .. }
                        | crate::base64_processor::Base64ProcessorError::InvalidBase64(_)
                )
            }
            _ => false, // Retry timeouts, memory issues, etc.
        }
    }

    /// Create fallback content for failed processing
    fn create_fallback_content(
        &self,
        index: usize,
        error: &ContentBlockProcessorError,
    ) -> ProcessedContent {
        let mut metadata = HashMap::new();
        metadata.insert("processing_failed".to_string(), "true".to_string());
        metadata.insert(
            "error_type".to_string(),
            format!("{:?}", std::mem::discriminant(error)),
        );
        metadata.insert("content_index".to_string(), index.to_string());

        ProcessedContent {
            content_type: ProcessedContentType::Text,
            text_representation: format!(
                "[Content processing failed at index {}: {}]",
                index, error
            ),
            binary_data: None,
            metadata,
            size_bytes: 0,
        }
    }

    /// Get content type key for counting
    fn get_content_type_key(&self, content_type: &ProcessedContentType) -> &str {
        match content_type {
            ProcessedContentType::Text => "text",
            ProcessedContentType::Image { .. } => "image",
            ProcessedContentType::Audio { .. } => "audio",
            ProcessedContentType::EmbeddedResource { .. } => "resource",
            ProcessedContentType::ResourceLink { .. } => "resource_link",
        }
    }
}

/// Summary of processing multiple content blocks
#[derive(Debug)]
pub struct ContentProcessingSummary {
    pub processed_contents: Vec<ProcessedContent>,
    pub combined_text: String,
    pub has_binary_content: bool,
    pub total_size_bytes: usize,
    pub content_type_counts: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{AudioContent, EmbeddedResource, ImageContent, ResourceLink};

    fn create_test_processor() -> ContentBlockProcessor {
        let mut supported_capabilities = HashMap::new();
        supported_capabilities.insert("text".to_string(), true);
        supported_capabilities.insert("image".to_string(), true);
        supported_capabilities.insert("audio".to_string(), true); // Enable for testing
        supported_capabilities.insert("resource".to_string(), true);
        supported_capabilities.insert("resource_link".to_string(), true);

        ContentBlockProcessor::new_with_config(
            Base64Processor::default(),
            50 * 1024 * 1024,        // 50MB for resources
            true,                    // enable_uri_validation
            Duration::from_secs(30), // processing_timeout
            true,                    // enable_capability_validation
            supported_capabilities,
            true, // enable_batch_recovery
        )
    }

    #[test]
    fn test_process_text_content() {
        let processor = create_test_processor();
        let text_content = TextContent {
            text: "Hello, world!".to_string(),
            annotations: None,
            meta: None,
        };

        let result = processor.process_text_content(&text_content);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.text_representation, "Hello, world!");
        assert_eq!(processed.size_bytes, 13);
        assert!(matches!(processed.content_type, ProcessedContentType::Text));
    }

    #[test]
    fn test_process_image_content_png() {
        let processor = create_test_processor();
        // 1x1 PNG in base64
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let image_content = ImageContent {
            data: png_data.to_string(),
            mime_type: "image/png".to_string(),
            uri: None,
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Image content: image/png"));
        assert!(processed.text_representation.contains("embedded"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::Image { .. }
        ));
        assert!(processed.binary_data.is_some());
        let binary_data = processed.binary_data.unwrap();
        assert!(!binary_data.is_empty());
    }

    #[test]
    fn test_process_image_content_with_uri() {
        let processor = create_test_processor();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let image_content = ImageContent {
            data: png_data.to_string(),
            mime_type: "image/png".to_string(),
            uri: Some("https://example.com/image.png".to_string()),
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("from https://example.com/image.png"));
        assert_eq!(
            processed.metadata.get("source_uri"),
            Some(&"https://example.com/image.png".to_string())
        );
    }

    #[test]
    fn test_process_audio_content_wav() {
        let processor = create_test_processor();

        // Test that audio capability is supported
        println!("Testing audio capability support...");
        let capability_result = processor.validate_capability("audio");
        if let Err(e) = &capability_result {
            println!("Audio capability validation failed: {:?}", e);
        }
        assert!(
            capability_result.is_ok(),
            "Audio capability should be supported in test processor"
        );

        // Simple WAV header in base64 (RIFF header + WAVE format)
        let wav_data = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";

        let audio_content = AudioContent {
            data: wav_data.to_string(),
            mime_type: "audio/wav".to_string(),
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::Audio(audio_content);

        // Test content block structure validation first
        println!("Testing content block structure validation...");
        let structure_result = processor.validate_content_block_structure(&content_block);
        if let Err(e) = &structure_result {
            println!("Structure validation failed: {:?}", e);
        }

        println!("Processing audio content block...");
        let result = processor.process_content_block(&content_block);

        match &result {
            Ok(_) => {
                println!("Audio processing succeeded");
            }
            Err(e) => {
                println!("Audio processing failed: {:?}", e);
                // Print the full error chain
                let mut current_error: &dyn std::error::Error = e;
                println!("Error chain:");
                println!("  - {}", current_error);
                while let Some(source) = current_error.source() {
                    println!("  - caused by: {}", source);
                    current_error = source;
                }
            }
        }

        assert!(
            result.is_ok(),
            "Expected audio processing to succeed, but got error: {:?}",
            result.err()
        );

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Audio content: audio/wav"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::Audio { .. }
        ));
        assert!(processed.binary_data.is_some());
    }

    #[test]
    fn test_process_resource_content_placeholder() {
        let processor = create_test_processor();

        // Create a proper EmbeddedResource with the actual structure
        let resource_data = serde_json::json!({
            "uri": "file:///test.txt",
            "text": "Test content",
            "mimeType": "text/plain"
        });
        let embedded_resource = EmbeddedResource {
            resource: serde_json::from_value(resource_data).unwrap(),
            annotations: None,
            meta: None,
        };

        // For now, we test with the placeholder implementation
        let content_block = ContentBlock::Resource(embedded_resource);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed.text_representation.contains("Embedded Resource"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::EmbeddedResource { .. }
        ));
    }

    #[test]
    fn test_process_resource_link_content() {
        let processor = create_test_processor();

        // Create a proper ResourceLink with the actual structure
        let resource_link = ResourceLink {
            uri: "https://example.com/document.pdf".to_string(),
            name: "document.pdf".to_string(),
            description: None,
            mime_type: None,
            title: None,
            size: None,
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::ResourceLink(resource_link);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Resource Link: https://example.com/document.pdf"));
        assert!(matches!(
            processed.content_type,
            ProcessedContentType::ResourceLink { .. }
        ));
        assert_eq!(processed.size_bytes, 0); // ResourceLink doesn't contain content data
    }

    #[test]
    fn test_validate_uri() {
        let processor = create_test_processor();

        assert!(processor.validate_uri("file:///test.txt").is_ok());
        assert!(processor.validate_uri("https://example.com").is_ok());
        assert!(processor.validate_uri("http://example.com").is_ok());
        assert!(processor
            .validate_uri("data:text/plain;base64,SGVsbG8=")
            .is_ok());

        // Error cases
        assert!(processor.validate_uri("").is_err());
        assert!(processor.validate_uri("invalid-uri").is_err());
        assert!(processor.validate_uri("just-a-path").is_err());
    }

    #[test]
    fn test_process_content_blocks_mixed() {
        let processor = create_test_processor();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        let content_blocks = vec![
            ContentBlock::Text(TextContent {
                text: "Hello".to_string(),
                annotations: None,
                meta: None,
            }),
            ContentBlock::Image(ImageContent {
                data: png_data.to_string(),
                mime_type: "image/png".to_string(),
                uri: None,
                annotations: None,
                meta: None,
            }),
        ];

        let result = processor.process_content_blocks(&content_blocks);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert_eq!(summary.processed_contents.len(), 2);
        assert!(summary.has_binary_content);
        assert_eq!(summary.content_type_counts.get("text"), Some(&1));
        assert_eq!(summary.content_type_counts.get("image"), Some(&1));
        assert!(summary.combined_text.contains("Hello"));
        assert!(summary.combined_text.contains("[Image content:"));
        assert!(summary.total_size_bytes > 0);
    }

    #[test]
    fn test_image_format_validation_error() {
        let processor = create_test_processor();
        // Invalid base64 data
        let invalid_data = "invalid-base64-data!@#$";

        let image_content = ImageContent {
            data: invalid_data.to_string(),
            mime_type: "image/png".to_string(),
            uri: None,
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_err());

        // Should be a base64 processing error
        assert!(matches!(
            result.unwrap_err(),
            ContentBlockProcessorError::Base64Error(_)
        ));
    }

    #[test]
    fn test_unsupported_mime_type() {
        let processor = create_test_processor();
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

        // Unsupported MIME type
        let image_content = ImageContent {
            data: png_data.to_string(),
            mime_type: "image/bmp".to_string(), // Not in allowed list
            uri: None,
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::Image(image_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_err());

        // Should be a MIME type error
        assert!(matches!(
            result.unwrap_err(),
            ContentBlockProcessorError::Base64Error(_)
        ));
    }

    #[test]
    fn test_uri_validation_disabled() {
        let processor = ContentBlockProcessor::new(
            Base64Processor::default(),
            50 * 1024 * 1024,
            false, // Disable URI validation
        );

        let resource_link = ResourceLink {
            uri: "invalid-scheme://test".to_string(),
            name: "test".to_string(),
            description: None,
            mime_type: None,
            title: None,
            size: None,
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::ResourceLink(resource_link);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok()); // Should pass with URI validation disabled
    }

    #[test]
    fn test_empty_content_blocks() {
        let processor = create_test_processor();
        let content_blocks = vec![];

        let result = processor.process_content_blocks(&content_blocks);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert_eq!(summary.processed_contents.len(), 0);
        assert!(!summary.has_binary_content);
        assert_eq!(summary.total_size_bytes, 0);
        assert!(summary.combined_text.is_empty());
        assert!(summary.content_type_counts.is_empty());
    }
}
