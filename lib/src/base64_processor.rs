use std::collections::HashSet;
use base64::{engine::general_purpose, Engine as _};
use thiserror::Error;

#[derive(Debug, Error)]
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
}

pub struct Base64Processor {
    max_size: usize,
    allowed_image_mime_types: HashSet<String>,
    allowed_audio_mime_types: HashSet<String>,
    allowed_blob_mime_types: HashSet<String>,
}

impl Default for Base64Processor {
    fn default() -> Self {
        let mut allowed_image_mime_types = HashSet::new();
        allowed_image_mime_types.insert("image/png".to_string());
        allowed_image_mime_types.insert("image/jpeg".to_string());
        allowed_image_mime_types.insert("image/gif".to_string());
        allowed_image_mime_types.insert("image/webp".to_string());

        let mut allowed_audio_mime_types = HashSet::new();
        allowed_audio_mime_types.insert("audio/wav".to_string());
        allowed_audio_mime_types.insert("audio/mp3".to_string());
        allowed_audio_mime_types.insert("audio/mpeg".to_string());
        allowed_audio_mime_types.insert("audio/ogg".to_string());
        allowed_audio_mime_types.insert("audio/aac".to_string());

        let mut allowed_blob_mime_types = HashSet::new();
        allowed_blob_mime_types.extend(allowed_image_mime_types.iter().cloned());
        allowed_blob_mime_types.extend(allowed_audio_mime_types.iter().cloned());
        allowed_blob_mime_types.insert("application/pdf".to_string());
        allowed_blob_mime_types.insert("text/plain".to_string());

        Self {
            max_size: 10 * 1024 * 1024, // 10MB default limit
            allowed_image_mime_types,
            allowed_audio_mime_types,
            allowed_blob_mime_types,
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

    pub fn decode_image_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        self.validate_mime_type(mime_type, &self.allowed_image_mime_types)?;
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        let decoded = general_purpose::STANDARD
            .decode(data)
            .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))?;

        self.validate_image_format(&decoded, mime_type)?;

        Ok(decoded)
    }

    pub fn decode_audio_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        self.validate_mime_type(mime_type, &self.allowed_audio_mime_types)?;
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        let decoded = general_purpose::STANDARD
            .decode(data)
            .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))?;

        self.validate_audio_format(&decoded, mime_type)?;

        Ok(decoded)
    }

    pub fn decode_blob_data(
        &self,
        data: &str,
        mime_type: &str,
    ) -> Result<Vec<u8>, Base64ProcessorError> {
        self.validate_mime_type(mime_type, &self.allowed_blob_mime_types)?;
        self.validate_base64_format(data)?;
        self.check_size_limits(data)?;

        let decoded = general_purpose::STANDARD
            .decode(data)
            .map_err(|e| Base64ProcessorError::InvalidBase64(e.to_string()))?;

        Ok(decoded)
    }

    fn validate_base64_format(&self, data: &str) -> Result<(), Base64ProcessorError> {
        if data.is_empty() {
            return Err(Base64ProcessorError::InvalidBase64(
                "Empty base64 data".to_string(),
            ));
        }

        // Check for invalid characters
        if !data.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c.is_whitespace()
        }) {
            return Err(Base64ProcessorError::InvalidBase64(
                "Contains invalid characters".to_string(),
            ));
        }

        // Check basic base64 padding rules
        let trimmed = data.trim();
        if !trimmed.len().is_multiple_of(4) {
            return Err(Base64ProcessorError::InvalidBase64(
                "Invalid base64 padding".to_string(),
            ));
        }

        Ok(())
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

    fn validate_image_format(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<(), Base64ProcessorError> {
        // Check minimum length based on format requirements
        let min_length = match mime_type {
            "image/jpeg" => 2,
            "image/gif" => 6,
            "image/png" => 8,
            "image/webp" => 12,
            _ => 8,
        };

        if data.len() < min_length {
            return Err(Base64ProcessorError::FormatMismatch {
                expected: mime_type.to_string(),
                actual: "insufficient data".to_string(),
            });
        }

        match mime_type {
            "image/png" => {
                if &data[0..8] != b"\x89PNG\r\n\x1a\n" {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "PNG".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            "image/jpeg" => {
                if &data[0..2] != b"\xFF\xD8" {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "JPEG".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            "image/gif" => {
                if data.len() < 6 || (&data[0..6] != b"GIF87a" && &data[0..6] != b"GIF89a") {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "GIF".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            "image/webp" => {
                if data.len() < 12
                    || &data[0..4] != b"RIFF"
                    || &data[8..12] != b"WEBP"
                {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "WebP".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            _ => {
                return Err(Base64ProcessorError::UnsupportedImageFormat(
                    mime_type.to_string(),
                ));
            }
        }

        Ok(())
    }

    fn validate_audio_format(
        &self,
        data: &[u8],
        mime_type: &str,
    ) -> Result<(), Base64ProcessorError> {
        if data.len() < 12 {
            return Err(Base64ProcessorError::FormatMismatch {
                expected: mime_type.to_string(),
                actual: "insufficient data".to_string(),
            });
        }

        match mime_type {
            "audio/wav" => {
                if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "WAV".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            "audio/mp3" | "audio/mpeg" => {
                // Check for MPEG frame sync bits
                if data.len() < 4 || (data[0] != 0xFF || (data[1] & 0xE0) != 0xE0) {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "MP3".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            "audio/ogg" => {
                if data.len() < 4 || &data[0..4] != b"OggS" {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "OGG".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            "audio/aac" => {
                // AAC ADTS sync word
                if data.len() < 7 || data[0] != 0xFF || (data[1] & 0xF0) != 0xF0 {
                    return Err(Base64ProcessorError::FormatMismatch {
                        expected: "AAC".to_string(),
                        actual: "unknown".to_string(),
                    });
                }
            }
            _ => {
                return Err(Base64ProcessorError::UnsupportedAudioFormat(
                    mime_type.to_string(),
                ));
            }
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
        let processor = Base64Processor::default();

        // Valid PNG header
        let png_header = b"\x89PNG\r\n\x1a\n";
        assert!(processor.validate_image_format(png_header, "image/png").is_ok());

        // Invalid PNG header
        let invalid_header = b"NOTPNG\x00\x00";
        assert!(processor.validate_image_format(invalid_header, "image/png").is_err());
    }

    #[test]
    fn test_validate_jpeg_format() {
        let processor = Base64Processor::default();

        // Valid JPEG header (SOI marker)
        let jpeg_header = b"\xFF\xD8\xFF\xE0";
        
        let result = processor.validate_image_format(jpeg_header, "image/jpeg");
        if let Err(e) = result {
            panic!("JPEG validation should have succeeded but got error: {}", e);
        }

        // Invalid JPEG header
        let invalid_header = b"NOTJPEG\x00";
        let result2 = processor.validate_image_format(invalid_header, "image/jpeg");
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

        // Test allowed image MIME type
        assert!(processor
            .validate_mime_type("image/png", &processor.allowed_image_mime_types)
            .is_ok());

        // Test disallowed MIME type
        assert!(processor
            .validate_mime_type("image/bmp", &processor.allowed_image_mime_types)
            .is_err());
    }
}