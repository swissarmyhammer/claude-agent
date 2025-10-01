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


## Proposed Solution

Based on code analysis, I will create a new `url_validation.rs` module following the same pattern as `validation_utils.rs`:

### Analysis Summary

**Location 1: session_validation.rs (lines 203-250)**
- Validates HTTP/SSE MCP server URLs
- Checks URL parsing and scheme (http/https only)
- No SSRF protection
- Error type: SessionSetupError

**Location 2: content_security_validator.rs (lines 546-627, 668-730)**
- Most comprehensive validation
- Checks: empty, size, URL parsing, scheme allowlist, blocked patterns, SSRF protection
- SSRF protection validates IP addresses (private IPv4/IPv6) and hostnames (localhost, metadata services)
- Error type: ContentSecurityError

**Location 3: content_block_processor.rs (lines 595-620)**
- Basic validation: empty, contains scheme
- Warns on unsupported schemes but doesn't fail
- Allows: file, http, https, data, ftp
- Error type: ContentBlockProcessorError

### Implementation Plan

1. **Create `lib/src/url_validation.rs`** with validation functions that return bool/Option, following validation_utils.rs pattern
   - `is_valid_url_format(url: &str) -> bool` - parse check
   - `is_allowed_scheme(url: &Url, schemes: &[&str]) -> bool` - scheme check
   - `is_private_ipv4(ip: &Ipv4Addr) -> bool` - IPv4 SSRF check
   - `is_private_ipv6(ip: &Ipv6Addr) -> bool` - IPv6 SSRF check
   - `is_ssrf_vulnerable_hostname(hostname: &str) -> bool` - hostname SSRF check
   - `validate_url_against_ssrf(url: &Url) -> Option<String>` - full SSRF validation

2. **Extract SSRF IP validation** from content_security_validator.rs:
   - Move `is_private_ipv4()` and `is_private_ipv6()` to url_validation.rs
   - Keep as public functions for reuse

3. **Update session_validation.rs**:
   - Replace URL parsing with url_validation functions
   - Convert validation results to SessionSetupError

4. **Update content_security_validator.rs**:
   - Replace URL validation with url_validation functions
   - Remove duplicate SSRF IP checking methods
   - Convert validation results to ContentSecurityError

5. **Update content_block_processor.rs**:
   - Replace URL validation with url_validation functions
   - Convert validation results to ContentBlockProcessorError

6. **Test with TDD**:
   - Run existing tests to ensure no regressions
   - Add new tests for url_validation.rs if needed

### Benefits
- Eliminates ~100 lines of duplicate code
- Consistent URL validation across codebase
- Centralized SSRF protection logic
- Follows existing validation_utils.rs pattern
- Each module retains domain-specific error types


## Implementation Complete

### Changes Made

1. **Created `lib/src/url_validation.rs`** (270 lines)
   - Follows validation_utils.rs pattern (returns bool/Option, not Result)
   - Public functions for URL validation without forcing error types
   - Functions:
     - `is_allowed_scheme(url, schemes)` - Check URL scheme against allowlist
     - `is_private_ipv4(ip)` - Detect private/reserved IPv4 addresses
     - `is_private_ipv6(ip)` - Detect private/reserved IPv6 addresses
     - `is_ssrf_vulnerable_hostname(hostname)` - Detect SSRF-vulnerable hostnames
     - `validate_url_against_ssrf(url)` - Full SSRF validation pipeline
   - Comprehensive test coverage (10 test cases)

2. **Updated `lib/src/session_validation.rs`**
   - Added `use crate::url_validation`
   - Replaced scheme validation in `validate_mcp_http_config()` with `url_validation::is_allowed_scheme()`
   - Replaced scheme validation in `validate_mcp_sse_config()` with `url_validation::is_allowed_scheme()`
   - Eliminated duplicate scheme checking logic

3. **Updated `lib/src/content_security_validator.rs`**
   - Added `use crate::url_validation`
   - Removed `use std::net::{Ipv4Addr, Ipv6Addr}` (no longer needed)
   - Replaced SSRF validation in `validate_uri_security()` with `url_validation::validate_url_against_ssrf()`
   - Removed 5 duplicate private methods (~70 lines):
     - `validate_ssrf_protection()`
     - `validate_ip_address()`
     - `validate_hostname()`
     - `is_private_ipv4()`
     - `is_private_ipv6()`

4. **Updated `lib/src/content_block_processor.rs`**
   - Added `use crate::url_validation` and `use url::Url`
   - Replaced manual scheme checking in `validate_uri()` with proper URL parsing + `url_validation::is_allowed_scheme()`
   - Improved validation from string checking to actual URL parsing

5. **Updated `lib/src/lib.rs`**
   - Added `pub mod url_validation;` to module exports

### Results

- **Compilation**: Clean build with no warnings
- **Tests**: All 547 tests pass (1 leaky test unrelated to changes)
- **Code Reduction**: Eliminated ~100 lines of duplicate URL validation and SSRF logic
- **Consistency**: All three modules now use the same URL validation functions
- **Maintainability**: Single source of truth for SSRF protection rules

### Design Decisions

1. Followed existing validation_utils.rs pattern for consistency
2. Functions return bool/Option rather than Result to allow domain-specific error types
3. Kept comprehensive doc comments with usage examples
4. Maintained all existing test coverage
5. SSRF validation detects:
   - Private IPv4: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
   - Loopback: 127.0.0.0/8
   - Link-local: 169.254.0.0/16
   - Private IPv6: ::1, ::, fe80::/10, fc00::/7
   - Localhost hostnames: localhost, 127.0.0.1, ::1
   - Cloud metadata services: 169.254.169.254, metadata.google.internal