# Consolidate URL/URI Validation Logic

## Problem
URL/URI parsing and validation appears in 3 different files with inconsistent validation depth and error handling.

## Locations

**Location 1:** `session_validation.rs:203-225`
- Basic format validation only
- Used for MCP HTTP and SSE configs
```rust
fn validate_mcp_http_config(config: &crate::config::HttpTransport) -> SessionSetupResult<()> {
    if Url::parse(&config.url).is_err() {
        return Err(SessionSetupError::McpServerConnectionFailed { ... });
    }
    Ok(())
}
```

**Location 2:** `content_security_validator.rs:426-461`
- Format + scheme validation + SSRF protection
- Most comprehensive validation
```rust
let parsed_uri = match Url::parse(uri) {
    Ok(url) => url,
    Err(_) => {
        return Err(ContentSecurityError::UriSecurityViolation { ... });
    }
};

let scheme = parsed_uri.scheme();
if !self.policy.allowed_uri_schemes.contains(scheme) {
    return Err(...);
}

if self.policy.enable_ssrf_protection {
    self.validate_ssrf_protection(&parsed_uri)?;
}
```

**Location 3:** `content_block_processor.rs:459`
- Basic URI validation
```rust
fn validate_uri(&self, uri: &str) -> Result<(), ContentBlockProcessorError>
```

## Recommendation

### Create Tiered URL Validation Module
**New file:** `lib/src/validation_utils/url.rs`

```rust
pub struct UrlValidator {
    allowed_schemes: HashSet<String>,
    enable_ssrf_protection: bool,
    max_length: usize,
}

impl UrlValidator {
    /// Basic format validation - parses URL
    pub fn validate_basic(&self, url: &str) -> Result<Url, ValidationError> {
        if url.is_empty() {
            return Err(ValidationError::EmptyUrl);
        }
        
        if url.len() > self.max_length {
            return Err(ValidationError::UrlTooLong { 
                actual: url.len(), 
                max: self.max_length 
            });
        }
        
        Url::parse(url).map_err(|_| ValidationError::InvalidUrlFormat(url.to_string()))
    }
    
    /// Validate URL scheme against allowlist
    pub fn validate_scheme(&self, url: &Url) -> Result<(), ValidationError> {
        if !self.allowed_schemes.contains(url.scheme()) {
            return Err(ValidationError::DisallowedScheme(url.scheme().to_string()));
        }
        Ok(())
    }
    
    /// SSRF protection - validate against private IPs and localhost
    pub fn validate_ssrf(&self, url: &Url) -> Result<(), ValidationError> {
        // IP address validation
        // Hostname validation
        // Private IP detection
    }
    
    /// Full validation pipeline
    pub fn validate_full(&self, url: &str) -> Result<Url, ValidationError> {
        let parsed = self.validate_basic(url)?;
        self.validate_scheme(&parsed)?;
        if self.enable_ssrf_protection {
            self.validate_ssrf(&parsed)?;
        }
        Ok(parsed)
    }
}
```

### Update Usage
- `session_validation.rs` - use `validate_basic()`
- `content_security_validator.rs` - use `validate_full()`
- `content_block_processor.rs` - use appropriate tier

## Impact
- Eliminates ~50 lines of duplication
- Consistent URL validation behavior
- Clear validation tiers for different security requirements
- Single SSRF protection implementation