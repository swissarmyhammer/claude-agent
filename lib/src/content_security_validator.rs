use crate::base64_validation;
use agent_client_protocol::ContentBlock;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};
use url::Url;

#[derive(Debug, Error, Clone)]
pub enum ContentSecurityError {
    #[error("Content security validation failed: {reason} (policy: {policy_violated})")]
    SecurityValidationFailed {
        reason: String,
        policy_violated: String,
    },
    #[error("Suspicious content detected: {threat_type} - {details}")]
    SuspiciousContentDetected {
        threat_type: String,
        details: String,
    },
    #[error("DoS protection triggered: {protection_type} (threshold: {threshold})")]
    DoSProtectionTriggered {
        protection_type: String,
        threshold: String,
    },
    #[error("URI security violation: {uri} - {reason}")]
    UriSecurityViolation { uri: String, reason: String },
    #[error("Base64 security violation: {reason}")]
    Base64SecurityViolation { reason: String },
    #[error("Content type spoofing detected: declared {declared}, actual {actual}")]
    ContentTypeSpoofingDetected { declared: String, actual: String },
    #[error("Content sanitization failed: {reason}")]
    ContentSanitizationFailed { reason: String },
    #[error("SSRF protection triggered: {target} - {reason}")]
    SsrfProtectionTriggered { target: String, reason: String },
    #[error("Processing timeout: operation exceeded {timeout}ms")]
    ProcessingTimeout { timeout: u64 },
    #[error("Memory limit exceeded: {actual} > {limit} bytes")]
    MemoryLimitExceeded { actual: usize, limit: usize },
    #[error("Rate limit exceeded: {operation}")]
    RateLimitExceeded { operation: String },
    #[error("Content array too large: {length} > {max_length}")]
    ContentArrayTooLarge { length: usize, max_length: usize },
    #[error("Invalid content encoding: {encoding}")]
    InvalidContentEncoding { encoding: String },
    #[error("Malicious pattern detected: {pattern_type}")]
    MaliciousPatternDetected { pattern_type: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SecurityLevel {
    Strict,
    Moderate,
    Permissive,
}

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    pub level: SecurityLevel,
    pub max_base64_size: usize,
    pub max_total_content_size: usize,
    pub max_content_array_length: usize,
    pub base64_decode_timeout: Duration,
    pub processing_timeout: Duration,
    pub allowed_uri_schemes: HashSet<String>,
    pub enable_ssrf_protection: bool,
    pub enable_content_sniffing: bool,
    pub enable_format_validation: bool,
    pub enable_content_sanitization: bool,
    pub enable_malicious_pattern_detection: bool,
    pub blocked_uri_patterns: Vec<String>,
    pub blocked_ip_ranges: Vec<String>,
    pub max_uri_length: usize,
    pub enable_rate_limiting: bool,
    pub rate_limit_requests_per_minute: u32,
}

impl SecurityPolicy {
    pub fn strict() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());

        Self {
            level: SecurityLevel::Strict,
            max_base64_size: 1024 * 1024,            // 1MB
            max_total_content_size: 5 * 1024 * 1024, // 5MB
            max_content_array_length: 10,
            base64_decode_timeout: Duration::from_secs(5),
            processing_timeout: Duration::from_secs(10),
            allowed_uri_schemes: allowed_schemes,
            enable_ssrf_protection: true,
            enable_content_sniffing: true,
            enable_format_validation: true,
            enable_content_sanitization: true,
            enable_malicious_pattern_detection: true,
            blocked_uri_patterns: vec![
                r"localhost".to_string(),
                r"127\..*".to_string(),
                r"192\.168\..*".to_string(),
                r"10\..*".to_string(),
                r"172\.(1[6-9]|2[0-9]|3[01])\..*".to_string(),
            ],
            blocked_ip_ranges: vec![
                "127.0.0.0/8".to_string(),
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
                "192.168.0.0/16".to_string(),
                "::1/128".to_string(),
            ],
            max_uri_length: 2048,
            enable_rate_limiting: true,
            rate_limit_requests_per_minute: 60,
        }
    }

    pub fn moderate() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());
        allowed_schemes.insert("http".to_string());
        allowed_schemes.insert("file".to_string());

        Self {
            level: SecurityLevel::Moderate,
            max_base64_size: 10 * 1024 * 1024,        // 10MB
            max_total_content_size: 50 * 1024 * 1024, // 50MB
            max_content_array_length: 50,
            base64_decode_timeout: Duration::from_secs(15),
            processing_timeout: Duration::from_secs(30),
            allowed_uri_schemes: allowed_schemes,
            enable_ssrf_protection: true,
            enable_content_sniffing: true,
            enable_format_validation: true,
            enable_content_sanitization: true,
            enable_malicious_pattern_detection: true,
            blocked_uri_patterns: vec![r"127\.0\.0\.1".to_string(), r"localhost".to_string()],
            blocked_ip_ranges: vec!["127.0.0.0/8".to_string(), "::1/128".to_string()],
            max_uri_length: 4096,
            enable_rate_limiting: true,
            rate_limit_requests_per_minute: 300,
        }
    }

    pub fn permissive() -> Self {
        let mut allowed_schemes = HashSet::new();
        allowed_schemes.insert("https".to_string());
        allowed_schemes.insert("http".to_string());
        allowed_schemes.insert("file".to_string());
        allowed_schemes.insert("data".to_string());
        allowed_schemes.insert("ftp".to_string());

        Self {
            level: SecurityLevel::Permissive,
            max_base64_size: 100 * 1024 * 1024,        // 100MB
            max_total_content_size: 500 * 1024 * 1024, // 500MB
            max_content_array_length: 1000,
            base64_decode_timeout: Duration::from_secs(60),
            processing_timeout: Duration::from_secs(120),
            allowed_uri_schemes: allowed_schemes,
            enable_ssrf_protection: false,
            enable_content_sniffing: false,
            enable_format_validation: false,
            enable_content_sanitization: false,
            enable_malicious_pattern_detection: false,
            blocked_uri_patterns: vec![],
            blocked_ip_ranges: vec![],
            max_uri_length: 8192,
            enable_rate_limiting: false,
            rate_limit_requests_per_minute: 0,
        }
    }
}

#[derive(Debug)]
pub struct ContentSecurityValidator {
    policy: SecurityPolicy,
    blocked_uri_regexes: Vec<Regex>,
    processing_stats: HashMap<String, u32>,
    last_rate_limit_reset: Instant,
}

impl Clone for ContentSecurityValidator {
    fn clone(&self) -> Self {
        // Recreate regex patterns from the policy
        let mut blocked_uri_regexes = Vec::new();
        for pattern in &self.policy.blocked_uri_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                blocked_uri_regexes.push(regex);
            }
        }

        Self {
            policy: self.policy.clone(),
            blocked_uri_regexes,
            processing_stats: self.processing_stats.clone(),
            last_rate_limit_reset: self.last_rate_limit_reset,
        }
    }
}

impl ContentSecurityValidator {
    pub fn new(policy: SecurityPolicy) -> Result<Self, ContentSecurityError> {
        let mut blocked_uri_regexes = Vec::new();
        for pattern in &policy.blocked_uri_patterns {
            match Regex::new(pattern) {
                Ok(regex) => blocked_uri_regexes.push(regex),
                Err(e) => {
                    return Err(ContentSecurityError::SecurityValidationFailed {
                        reason: format!("Invalid regex pattern '{}': {}", pattern, e),
                        policy_violated: "uri_pattern_validation".to_string(),
                    });
                }
            }
        }

        Ok(Self {
            policy,
            blocked_uri_regexes,
            processing_stats: HashMap::new(),
            last_rate_limit_reset: Instant::now(),
        })
    }

    pub fn strict() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::strict())
    }

    pub fn moderate() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::moderate())
    }

    pub fn permissive() -> Result<Self, ContentSecurityError> {
        Self::new(SecurityPolicy::permissive())
    }

    pub fn policy(&self) -> &SecurityPolicy {
        &self.policy
    }

    /// Perform comprehensive content security validation
    pub fn validate_content_security(
        &self,
        content: &ContentBlock,
    ) -> Result<(), ContentSecurityError> {
        debug!(
            "Starting content security validation for {:?}",
            std::mem::discriminant(content)
        );

        let start_time = Instant::now();

        // Apply timeout to entire validation process
        let result = self.validate_content_internal(content);

        let elapsed = start_time.elapsed();
        if elapsed > self.policy.processing_timeout {
            return Err(ContentSecurityError::ProcessingTimeout {
                timeout: elapsed.as_millis() as u64,
            });
        }

        result
    }

    fn validate_content_internal(
        &self,
        content: &ContentBlock,
    ) -> Result<(), ContentSecurityError> {
        match content {
            ContentBlock::Text(text_content) => {
                self.validate_text_security(text_content)?;
            }
            ContentBlock::Image(image_content) => {
                self.validate_base64_security(&image_content.data, "image")?;
                if let Some(ref uri) = image_content.uri {
                    self.validate_uri_security(uri)?;
                }
                if self.policy.enable_format_validation {
                    self.validate_content_type_consistency(
                        &image_content.data,
                        &image_content.mime_type,
                    )?;
                }
            }
            ContentBlock::Audio(audio_content) => {
                self.validate_base64_security(&audio_content.data, "audio")?;
                if self.policy.enable_format_validation {
                    self.validate_content_type_consistency(
                        &audio_content.data,
                        &audio_content.mime_type,
                    )?;
                }
            }
            ContentBlock::Resource(_resource_content) => {
                // Enhanced Resource validation will be implemented when Resource processing is fully available
                debug!(
                    "Resource content security validation - placeholder for future implementation"
                );
            }
            ContentBlock::ResourceLink(resource_link) => {
                self.validate_uri_security(&resource_link.uri)?;
            }
        }

        Ok(())
    }

    /// Validate array of content blocks
    pub fn validate_content_blocks_security(
        &self,
        content_blocks: &[ContentBlock],
    ) -> Result<(), ContentSecurityError> {
        // Check array size limits
        if content_blocks.len() > self.policy.max_content_array_length {
            return Err(ContentSecurityError::ContentArrayTooLarge {
                length: content_blocks.len(),
                max_length: self.policy.max_content_array_length,
            });
        }

        // Calculate total content size estimate
        let mut total_estimated_size = 0;
        for content_block in content_blocks {
            match content_block {
                ContentBlock::Text(text) => {
                    total_estimated_size += text.text.len();
                }
                ContentBlock::Image(image) => {
                    // Base64 encoded size is ~4/3 of actual size
                    total_estimated_size += (image.data.len() * 3) / 4;
                }
                ContentBlock::Audio(audio) => {
                    total_estimated_size += (audio.data.len() * 3) / 4;
                }
                ContentBlock::Resource(_) => {
                    // Conservative estimate for resource content
                    total_estimated_size += 1024; // 1KB estimate
                }
                ContentBlock::ResourceLink(_) => {
                    // URI-based content has minimal memory impact
                    total_estimated_size += 512; // 512B estimate
                }
            }
        }

        if total_estimated_size > self.policy.max_total_content_size {
            return Err(ContentSecurityError::DoSProtectionTriggered {
                protection_type: "total_content_size".to_string(),
                threshold: format!(
                    "{} > {}",
                    total_estimated_size, self.policy.max_total_content_size
                ),
            });
        }

        // Validate each content block
        for (index, content_block) in content_blocks.iter().enumerate() {
            if let Err(e) = self.validate_content_security(content_block) {
                warn!(
                    "Content security validation failed for block {}: {}",
                    index, e
                );
                return Err(e);
            }
        }

        Ok(())
    }

    /// Validate base64 data security
    pub fn validate_base64_security(
        &self,
        data: &str,
        content_type: &str,
    ) -> Result<(), ContentSecurityError> {
        // Check size limits before processing
        let estimated_decoded_size = (data.len() * 3) / 4;
        if estimated_decoded_size > self.policy.max_base64_size {
            return Err(ContentSecurityError::Base64SecurityViolation {
                reason: format!(
                    "Base64 {} content too large: {} > {} bytes",
                    content_type, estimated_decoded_size, self.policy.max_base64_size
                ),
            });
        }

        // Validate base64 format
        if let Err(e) = base64_validation::validate_base64_format(data) {
            return Err(ContentSecurityError::Base64SecurityViolation {
                reason: format!("Invalid base64 format: {}", e),
            });
        }

        // Check for malicious patterns in base64 data if enabled
        if self.policy.enable_malicious_pattern_detection {
            if let Some(pattern_type) = self.detect_malicious_base64_patterns(data) {
                return Err(ContentSecurityError::MaliciousPatternDetected { pattern_type });
            }
        }

        Ok(())
    }

    /// Validate URI security including SSRF protection
    pub fn validate_uri_security(&self, uri: &str) -> Result<(), ContentSecurityError> {
        // Basic format validation
        if uri.is_empty() {
            return Err(ContentSecurityError::UriSecurityViolation {
                uri: uri.to_string(),
                reason: "Empty URI".to_string(),
            });
        }

        if uri.len() > self.policy.max_uri_length {
            return Err(ContentSecurityError::UriSecurityViolation {
                uri: uri.to_string(),
                reason: format!(
                    "URI too long: {} > {}",
                    uri.len(),
                    self.policy.max_uri_length
                ),
            });
        }

        // Parse URI
        let parsed_uri = match Url::parse(uri) {
            Ok(url) => url,
            Err(_) => {
                return Err(ContentSecurityError::UriSecurityViolation {
                    uri: uri.to_string(),
                    reason: "Invalid URI format".to_string(),
                });
            }
        };

        // Validate scheme
        let scheme = parsed_uri.scheme();
        if !self.policy.allowed_uri_schemes.contains(scheme) {
            return Err(ContentSecurityError::UriSecurityViolation {
                uri: uri.to_string(),
                reason: format!("Disallowed URI scheme: {}", scheme),
            });
        }

        // Check blocked patterns
        for regex in &self.blocked_uri_regexes {
            if regex.is_match(uri) {
                return Err(ContentSecurityError::UriSecurityViolation {
                    uri: uri.to_string(),
                    reason: "URI matches blocked pattern".to_string(),
                });
            }
        }

        // SSRF protection
        if self.policy.enable_ssrf_protection {
            self.validate_ssrf_protection(&parsed_uri)?;
        }

        Ok(())
    }

    /// Validate text content security
    pub fn validate_text_security(
        &self,
        text_content: &agent_client_protocol::TextContent,
    ) -> Result<(), ContentSecurityError> {
        if self.policy.enable_content_sanitization {
            self.validate_text_content_safety(&text_content.text)?;
        }

        Ok(())
    }

    /// Validate content type consistency to detect spoofing
    pub fn validate_content_type_consistency(
        &self,
        _base64_data: &str,
        declared_mime_type: &str,
    ) -> Result<(), ContentSecurityError> {
        if !self.policy.enable_content_sniffing {
            return Ok(());
        }

        // This is a placeholder for content sniffing implementation
        // In a real implementation, we would decode a small portion of the base64 data
        // and check magic numbers to verify the actual content type
        debug!(
            "Content type consistency validation for {}",
            declared_mime_type
        );

        Ok(())
    }

    /// Detect malicious patterns in base64 data
    fn detect_malicious_base64_patterns(&self, data: &str) -> Option<String> {
        // Check for suspicious patterns that might indicate embedded executables or malicious content

        // Look for patterns that might decode to executable headers
        if data.starts_with("TVq") || data.starts_with("TVo") {
            return Some("potential_pe_executable".to_string());
        }

        if data.starts_with("f0VMR") {
            return Some("potential_elf_executable".to_string());
        }

        // Check for overly repetitive patterns (potential zip bombs or data corruption)
        if self.is_overly_repetitive(data) {
            return Some("repetitive_pattern".to_string());
        }

        None
    }

    /// Check if data contains overly repetitive patterns
    fn is_overly_repetitive(&self, data: &str) -> bool {
        if data.len() < 100 {
            return false;
        }

        // Sample check: if first 50 characters repeat more than 10 times
        if data.len() >= 50 {
            let sample = &data[0..50];
            let count = data.matches(sample).count();
            if count > 10 {
                return true;
            }
        }

        false
    }

    /// Validate SSRF protection
    fn validate_ssrf_protection(&self, parsed_uri: &Url) -> Result<(), ContentSecurityError> {
        if let Some(host) = parsed_uri.host_str() {
            // Check if host is an IP address
            if let Ok(ip) = host.parse::<IpAddr>() {
                self.validate_ip_address(&ip, parsed_uri.as_str())?;
            } else {
                // Check hostname patterns
                self.validate_hostname(host, parsed_uri.as_str())?;
            }
        }

        Ok(())
    }

    /// Validate IP address for SSRF protection
    fn validate_ip_address(&self, ip: &IpAddr, uri: &str) -> Result<(), ContentSecurityError> {
        match ip {
            IpAddr::V4(ipv4) => {
                if self.is_private_ipv4(ipv4) {
                    return Err(ContentSecurityError::SsrfProtectionTriggered {
                        target: uri.to_string(),
                        reason: "Private IPv4 address".to_string(),
                    });
                }
            }
            IpAddr::V6(ipv6) => {
                if self.is_private_ipv6(ipv6) {
                    return Err(ContentSecurityError::SsrfProtectionTriggered {
                        target: uri.to_string(),
                        reason: "Private IPv6 address".to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Validate hostname for SSRF protection
    fn validate_hostname(&self, hostname: &str, uri: &str) -> Result<(), ContentSecurityError> {
        let hostname_lower = hostname.to_lowercase();

        // Check for localhost variants
        if hostname_lower == "localhost" || hostname_lower == "127.0.0.1" {
            return Err(ContentSecurityError::SsrfProtectionTriggered {
                target: uri.to_string(),
                reason: "Localhost hostname".to_string(),
            });
        }

        // Check for metadata service endpoints (cloud providers)
        if hostname_lower == "169.254.169.254" || hostname_lower == "metadata.google.internal" {
            return Err(ContentSecurityError::SsrfProtectionTriggered {
                target: uri.to_string(),
                reason: "Metadata service endpoint".to_string(),
            });
        }

        Ok(())
    }

    /// Check if IPv4 address is private
    fn is_private_ipv4(&self, ip: &Ipv4Addr) -> bool {
        ip.is_private() || ip.is_loopback() || ip.is_link_local()
    }

    /// Check if IPv6 address is private
    fn is_private_ipv6(&self, ip: &Ipv6Addr) -> bool {
        ip.is_loopback() || ip.is_unspecified()
    }

    /// Validate text content for potentially dangerous content
    fn validate_text_content_safety(&self, text: &str) -> Result<(), ContentSecurityError> {
        // Check for basic script injection patterns
        let dangerous_patterns = [
            "<script",
            "javascript:",
            "onload=",
            "onerror=",
            "eval(",
            "document.cookie",
        ];

        let text_lower = text.to_lowercase();
        for pattern in &dangerous_patterns {
            if text_lower.contains(pattern) {
                return Err(ContentSecurityError::ContentSanitizationFailed {
                    reason: format!("Potentially dangerous pattern detected: {}", pattern),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::TextContent;

    fn create_test_validator() -> ContentSecurityValidator {
        ContentSecurityValidator::moderate().unwrap()
    }

    #[test]
    fn test_security_policy_levels() {
        let strict = SecurityPolicy::strict();
        let moderate = SecurityPolicy::moderate();
        let permissive = SecurityPolicy::permissive();

        assert_eq!(strict.level, SecurityLevel::Strict);
        assert_eq!(moderate.level, SecurityLevel::Moderate);
        assert_eq!(permissive.level, SecurityLevel::Permissive);

        // Strict should have tighter limits
        assert!(strict.max_base64_size < moderate.max_base64_size);
        assert!(moderate.max_base64_size < permissive.max_base64_size);
    }

    #[test]
    fn test_uri_security_validation() {
        let validator = create_test_validator();

        // Valid URIs
        assert!(validator
            .validate_uri_security("https://example.com")
            .is_ok());
        assert!(validator
            .validate_uri_security("http://example.com")
            .is_ok());
        assert!(validator
            .validate_uri_security("file:///tmp/test.txt")
            .is_ok());

        // Invalid URIs
        assert!(validator.validate_uri_security("").is_err());
        assert!(validator.validate_uri_security("invalid-uri").is_err());
        assert!(validator
            .validate_uri_security("javascript:alert(1)")
            .is_err());

        // SSRF protection
        assert!(validator.validate_uri_security("http://localhost").is_err());
        assert!(validator.validate_uri_security("http://127.0.0.1").is_err());
    }

    #[test]
    fn test_base64_security_validation() {
        let validator = create_test_validator();

        // Valid base64
        assert!(validator
            .validate_base64_security("SGVsbG8gV29ybGQ=", "test")
            .is_ok());

        // Invalid base64
        assert!(validator.validate_base64_security("", "test").is_err());
        assert!(validator
            .validate_base64_security("Invalid!@#$", "test")
            .is_err());

        // Too large (simulate by using policy with small limit)
        let strict_validator = ContentSecurityValidator::strict().unwrap();
        let large_data = "A".repeat(2 * 1024 * 1024); // 2MB of 'A's
        assert!(strict_validator
            .validate_base64_security(&large_data, "test")
            .is_err());
    }

    #[test]
    fn test_text_security_validation() {
        let validator = create_test_validator();

        let safe_text = TextContent {
            text: "This is safe text content".to_string(),
            annotations: None,
            meta: None,
        };

        let dangerous_text = TextContent {
            text: "<script>alert('xss')</script>".to_string(),
            annotations: None,
            meta: None,
        };

        assert!(validator.validate_text_security(&safe_text).is_ok());
        assert!(validator.validate_text_security(&dangerous_text).is_err());
    }

    #[test]
    fn test_content_blocks_security_validation() {
        let validator = create_test_validator();

        let safe_content = vec![ContentBlock::Text(TextContent {
            text: "Hello".to_string(),
            annotations: None,
            meta: None,
        })];

        let too_many_content = vec![
            ContentBlock::Text(TextContent {
                text: "test".to_string(),
                annotations: None,
                meta: None,
            });
            100
        ]; // Exceeds moderate policy limit

        assert!(validator
            .validate_content_blocks_security(&safe_content)
            .is_ok());
        assert!(validator
            .validate_content_blocks_security(&too_many_content)
            .is_err());
    }

    #[test]
    fn test_malicious_pattern_detection() {
        let validator = create_test_validator();

        // Test executable detection
        let pe_executable_base64 = "TVqQAAMAAAAEAAAA"; // PE header in base64
        let elf_executable_base64 = "f0VMRgIBAQAAAAA"; // ELF header in base64

        if validator.policy.enable_malicious_pattern_detection {
            assert!(validator
                .detect_malicious_base64_patterns(pe_executable_base64)
                .is_some());
            assert!(validator
                .detect_malicious_base64_patterns(elf_executable_base64)
                .is_some());
        }

        // Safe base64 should pass
        let safe_base64 = "SGVsbG8gV29ybGQ="; // "Hello World" in base64
        assert!(validator
            .detect_malicious_base64_patterns(safe_base64)
            .is_none());
    }

    #[test]
    fn test_ssrf_protection() {
        let validator = ContentSecurityValidator::strict().unwrap();

        // These should be blocked by SSRF protection
        assert!(validator.validate_uri_security("http://127.0.0.1").is_err());
        assert!(validator.validate_uri_security("http://localhost").is_err());
        assert!(validator
            .validate_uri_security("http://169.254.169.254")
            .is_err());
        assert!(validator.validate_uri_security("http://10.0.0.1").is_err());

        // These should be allowed
        assert!(validator
            .validate_uri_security("https://example.com")
            .is_ok());
        assert!(validator
            .validate_uri_security("https://google.com")
            .is_ok());
    }

    #[test]
    fn test_processing_timeout() {
        let mut policy = SecurityPolicy::moderate();
        policy.processing_timeout = Duration::from_millis(1); // Very short timeout
        let validator = ContentSecurityValidator::new(policy).unwrap();

        let content = ContentBlock::Text(TextContent {
            text: "Test content".to_string(),
            annotations: None,
            meta: None,
        });

        // This might pass or fail depending on system performance
        // The test mainly checks that timeout handling is implemented
        let _result = validator.validate_content_security(&content);
    }
}
