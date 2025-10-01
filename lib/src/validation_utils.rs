//! Common validation utilities for consistent validation patterns across the codebase
//!
//! This module provides reusable validation functions to standardize empty/null value
//! checking without forcing a common error type. Each validation function returns a bool
//! or Option, allowing callsites to use their domain-specific error types.
//!
//! ## Design Decision
//!
//! These validation functions return bool or `Option<String>` rather than `Result`
//! to allow call sites to use their domain-specific error types while still
//! centralizing the validation logic. This approach:
//!
//! - Eliminates code duplication across the codebase
//! - Maintains semantic correctness of domain-specific errors
//! - Avoids breaking changes to existing error handling
//! - Provides a consistent validation pattern
//!
//! ## Usage Examples
//!
//! ### With PathValidationError
//!
//! ```ignore
//! use crate::validation_utils;
//! use crate::path_validator::PathValidationError;
//!
//! fn validate_path(path_str: &str) -> Result<(), PathValidationError> {
//!     if validation_utils::is_empty_str(path_str) {
//!         return Err(PathValidationError::EmptyPath);
//!     }
//!     // ... rest of validation
//!     Ok(())
//! }
//! ```
//!
//! ### With SessionSetupError
//!
//! ```ignore
//! use crate::validation_utils;
//! use crate::session::SessionSetupError;
//!
//! fn validate_session_id(session_id: &str) -> Result<(), SessionSetupError> {
//!     if validation_utils::is_empty_str(session_id) {
//!         return Err(SessionSetupError::MissingRequiredParameter {
//!             parameter_name: "session_id".to_string(),
//!         });
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### With ContentSecurityError
//!
//! ```ignore
//! use crate::validation_utils;
//! use crate::content_security_validator::ContentSecurityError;
//!
//! fn validate_uri(uri: &str) -> Result<(), ContentSecurityError> {
//!     if validation_utils::is_empty_str(uri) {
//!         return Err(ContentSecurityError::UriSecurityViolation {
//!             uri: uri.to_string(),
//!             reason: "URI cannot be empty".to_string(),
//!         });
//!     }
//!     Ok(())
//! }
//! ```

use std::path::Path;

/// Check if a string is empty
///
/// This is marked inline to ensure zero-cost abstraction over `is_empty()`.
/// The function compiles down to the same code as calling `is_empty()` directly,
/// but provides a centralized location for consistent validation patterns.
///
/// # Examples
///
/// ```
/// use claude_agent_lib::validation_utils::is_empty_str;
///
/// assert!(is_empty_str(""));
/// assert!(!is_empty_str("hello"));
/// ```
#[inline]
pub fn is_empty_str(value: &str) -> bool {
    value.is_empty()
}

/// Check if a path is empty
///
/// A path is considered empty if its OsStr representation is empty.
/// This is marked inline for zero-cost abstraction.
///
/// # Examples
///
/// ```
/// use claude_agent_lib::validation_utils::is_empty_path;
/// use std::path::Path;
///
/// assert!(is_empty_path(Path::new("")));
/// assert!(!is_empty_path(Path::new("/tmp")));
/// ```
#[inline]
pub fn is_empty_path(path: &Path) -> bool {
    path.as_os_str().is_empty()
}

/// Validate that a string is not empty, returning None if valid or Some(reason) if invalid
///
/// This allows callsites to check emptiness and convert to their own error types.
///
/// # Examples
///
/// ```
/// use claude_agent_lib::validation_utils::validate_not_empty_str;
///
/// assert!(validate_not_empty_str("hello", "username").is_none());
/// assert_eq!(
///     validate_not_empty_str("", "username"),
///     Some("username cannot be empty".to_string())
/// );
/// ```
pub fn validate_not_empty_str(value: &str, field_name: &str) -> Option<String> {
    if value.is_empty() {
        Some(format!("{} cannot be empty", field_name))
    } else {
        None
    }
}

/// Validate that a path is not empty, returning None if valid or Some(reason) if invalid
///
/// # Examples
///
/// ```
/// use claude_agent_lib::validation_utils::validate_not_empty_path;
/// use std::path::Path;
///
/// assert!(validate_not_empty_path(Path::new("/tmp"), "cwd").is_none());
/// assert_eq!(
///     validate_not_empty_path(Path::new(""), "cwd"),
///     Some("cwd cannot be empty".to_string())
/// );
/// ```
pub fn validate_not_empty_path(path: &Path, field_name: &str) -> Option<String> {
    if is_empty_path(path) {
        Some(format!("{} cannot be empty", field_name))
    } else {
        None
    }
}

/// Check if a string exceeds a maximum length
///
/// This is marked inline for zero-cost abstraction.
///
/// # Examples
///
/// ```
/// use claude_agent_lib::validation_utils::exceeds_max_length;
///
/// assert!(exceeds_max_length("hello", 3));
/// assert!(!exceeds_max_length("hi", 3));
/// ```
#[inline]
pub fn exceeds_max_length(value: &str, max_length: usize) -> bool {
    value.len() > max_length
}

/// Validate maximum length, returning None if valid or Some(reason) if invalid
///
/// # Examples
///
/// ```
/// use claude_agent_lib::validation_utils::validate_max_length;
///
/// assert!(validate_max_length("hi", 10, "username").is_none());
/// let result = validate_max_length("hello", 3, "username");
/// assert!(result.is_some());
/// assert!(result.unwrap().contains("username"));
/// assert!(result.unwrap().contains("5"));
/// assert!(result.unwrap().contains("3"));
/// ```
pub fn validate_max_length(value: &str, max_length: usize, field_name: &str) -> Option<String> {
    if exceeds_max_length(value, max_length) {
        Some(format!(
            "{} exceeds maximum length: {} > {}",
            field_name,
            value.len(),
            max_length
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_empty_str() {
        assert!(is_empty_str(""));
        assert!(!is_empty_str("hello"));
        assert!(!is_empty_str(" "));
        assert!(!is_empty_str("\n"));
    }

    #[test]
    fn test_is_empty_path() {
        assert!(is_empty_path(Path::new("")));
        assert!(!is_empty_path(Path::new("/")));
        assert!(!is_empty_path(Path::new("/tmp")));
        assert!(!is_empty_path(Path::new("relative/path")));
    }

    #[test]
    fn test_validate_not_empty_str() {
        // Valid cases
        assert!(validate_not_empty_str("hello", "username").is_none());
        assert!(validate_not_empty_str("x", "id").is_none());

        // Invalid cases
        let result = validate_not_empty_str("", "username");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "username cannot be empty");

        let result = validate_not_empty_str("", "session_id");
        assert!(result.is_some());
        assert!(result.unwrap().contains("session_id"));
    }

    #[test]
    fn test_validate_not_empty_path() {
        // Valid cases
        assert!(validate_not_empty_path(Path::new("/tmp"), "cwd").is_none());
        assert!(validate_not_empty_path(Path::new("relative"), "path").is_none());

        // Invalid cases
        let result = validate_not_empty_path(Path::new(""), "cwd");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "cwd cannot be empty");

        let result = validate_not_empty_path(&PathBuf::from(""), "working_dir");
        assert!(result.is_some());
        assert!(result.unwrap().contains("working_dir"));
    }

    #[test]
    fn test_exceeds_max_length() {
        assert!(exceeds_max_length("hello", 4));
        assert!(!exceeds_max_length("hello", 5));
        assert!(!exceeds_max_length("hello", 10));
        assert!(!exceeds_max_length("", 0)); // Edge case: empty string has length 0, which equals 0
        assert!(exceeds_max_length("x", 0)); // Single char exceeds 0 limit
    }

    #[test]
    fn test_validate_max_length() {
        // Valid cases
        assert!(validate_max_length("hello", 10, "username").is_none());
        assert!(validate_max_length("", 10, "optional").is_none());

        // Invalid cases
        let result = validate_max_length("hello world", 5, "username");
        assert!(result.is_some());
        let msg = result.unwrap();
        assert!(msg.contains("username"));
        assert!(msg.contains("11"));
        assert!(msg.contains("5"));
    }

    #[test]
    fn test_empty_path_edge_cases() {
        // Test with PathBuf
        let empty_pathbuf = PathBuf::from("");
        assert!(is_empty_path(&empty_pathbuf));

        // Test with Path reference
        let path_ref = Path::new("");
        assert!(is_empty_path(path_ref));
    }
}
