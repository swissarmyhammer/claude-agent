use crate::base64_processor::{Base64Processor, Base64ProcessorError};
use agent_client_protocol::{ContentBlock, TextContent};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, error, warn};

#[derive(Debug, Error)]
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
    max_resource_size: usize,
    enable_uri_validation: bool,
}

impl Default for ContentBlockProcessor {
    fn default() -> Self {
        Self {
            base64_processor: Base64Processor::default(),
            max_resource_size: 50 * 1024 * 1024, // 50MB for resources
            enable_uri_validation: true,
        }
    }
}

impl ContentBlockProcessor {
    pub fn new(
        base64_processor: Base64Processor,
        max_resource_size: usize,
        enable_uri_validation: bool,
    ) -> Self {
        Self {
            base64_processor,
            max_resource_size,
            enable_uri_validation,
        }
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

        match content_block {
            ContentBlock::Text(text_content) => self.process_text_content(text_content),
            ContentBlock::Image(image_content) => {
                // Decode and validate image data using existing base64_processor
                let decoded_data = self
                    .base64_processor
                    .decode_image_data(&image_content.data, &image_content.mime_type)?;

                // Check resource size limit
                if decoded_data.len() > self.max_resource_size {
                    return Err(ContentBlockProcessorError::ContentSizeExceeded {
                        actual: decoded_data.len(),
                        limit: self.max_resource_size,
                    });
                }

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
                if decoded_data.len() > self.max_resource_size {
                    return Err(ContentBlockProcessorError::ContentSizeExceeded {
                        actual: decoded_data.len(),
                        limit: self.max_resource_size,
                    });
                }

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

    /// Get comprehensive content processing summary for all content blocks
    pub fn process_content_blocks(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<ContentProcessingSummary, ContentBlockProcessorError> {
        let mut text_content = String::new();
        let mut has_binary_content = false;
        let mut processed_contents = Vec::new();
        let mut total_size = 0;
        let mut content_type_counts = HashMap::new();

        for content_block in content_blocks {
            let processed = self.process_content_block(content_block)?;

            // Accumulate text representation
            text_content.push_str(&processed.text_representation);

            // Track binary content
            if processed.binary_data.is_some() {
                has_binary_content = true;
            }

            // Update size and type counts
            total_size += processed.size_bytes;
            let type_key = match &processed.content_type {
                ProcessedContentType::Text => "text",
                ProcessedContentType::Image { .. } => "image",
                ProcessedContentType::Audio { .. } => "audio",
                ProcessedContentType::EmbeddedResource { .. } => "resource",
                ProcessedContentType::ResourceLink { .. } => "resource_link",
            };
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
}

/// Summary of processing multiple content blocks
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
        ContentBlockProcessor::default()
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
        // Simple WAV header in base64 (RIFF header + WAVE format)
        let wav_data = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";

        let audio_content = AudioContent {
            data: wav_data.to_string(),
            mime_type: "audio/wav".to_string(),
            annotations: None,
            meta: None,
        };

        let content_block = ContentBlock::Audio(audio_content);
        let result = processor.process_content_block(&content_block);
        assert!(result.is_ok());

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
