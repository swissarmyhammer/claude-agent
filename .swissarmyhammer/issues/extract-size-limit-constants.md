# Extract Size Limit Constants

## Problem
Size limits are scattered throughout the codebase as magic numbers with inconsistent values:
- Path lengths: 1024, 4096
- Content sizes: 1MB, 5MB, 10MB, 50MB, 100MB, 500MB
- Buffer sizes: 32, 100, 1000
- Other limits: 100_000 (meta size), 10000 (history messages)

## Locations

**Path limits:**
- `path_validator.rs:56` - `max_path_length: 4096`
- `path_validator.rs:460` - `1024`

**Content size limits:**
- `base64_processor.rs:75` - `10 * 1024 * 1024` (10MB)
- `content_security_validator.rs:86` - `1024 * 1024` (1MB)
- `content_security_validator.rs:87` - `5 * 1024 * 1024` (5MB)
- `content_security_validator.rs:125` - `10 * 1024 * 1024` (10MB)
- `content_security_validator.rs:126` - `50 * 1024 * 1024` (50MB)
- `content_security_validator.rs:154` - `100 * 1024 * 1024` (100MB)
- `content_security_validator.rs:155` - `500 * 1024 * 1024` (500MB)
- `content_block_processor.rs:108` - `50 * 1024 * 1024` (50MB)
- `agent.rs:483` - `50 * 1024 * 1024` (50MB)

**URI limits:**
- `content_security_validator.rs:138` - `4096`
- `content_security_validator.rs:167` - `8192`

**Meta/config limits:**
- `request_validation.rs:142` - `100_000` (hardcoded!)
- `config.rs:7` - `100_000` (max prompt length)
- `config.rs:12` - `1000` (notification buffer)
- `config.rs:17` - `100` (cancellation buffer)
- `config.rs:22` - `100_000` (max tokens per turn)

**Buffer sizes:**
- Multiple files - `32` for notification channels
- `server.rs:467` - `1024` for duplex stream

## Issues
1. **Inconsistent values** - Same concepts have different limits in different places
2. **No semantic meaning** - Hard to know what 50 * 1024 * 1024 represents
3. **Difficult to maintain** - Changing limits requires hunting through code
4. **No relationship documented** - Why is strict mode 1MB but moderate 10MB?

## Recommendation

### Create Size Constants Module
**New file:** `lib/src/constants/sizes.rs`

```rust
//! Size limit constants for the agent

/// File system limits
pub mod fs {
    /// Maximum path length (4KB)
    pub const MAX_PATH_LENGTH: usize = 4096;
    
    /// Strict path length limit for sensitive operations (1KB)
    pub const MAX_PATH_LENGTH_STRICT: usize = 1024;
}

/// URI and URL limits
pub mod uri {
    /// Standard maximum URI length (4KB)
    pub const MAX_URI_LENGTH: usize = 4096;
    
    /// Extended URI length for permissive mode (8KB)
    pub const MAX_URI_LENGTH_EXTENDED: usize = 8192;
}

/// Content size limits by security level
pub mod content {
    /// Base unit for content sizes (1KB)
    pub const KB: usize = 1024;
    
    /// Base unit for content sizes (1MB)
    pub const MB: usize = 1024 * KB;
    
    /// Strict mode content limit (1MB)
    pub const MAX_CONTENT_STRICT: usize = 1 * MB;
    
    /// Moderate mode content limit (10MB)
    pub const MAX_CONTENT_MODERATE: usize = 10 * MB;
    
    /// Permissive mode content limit (100MB)
    pub const MAX_CONTENT_PERMISSIVE: usize = 100 * MB;
    
    /// Strict mode resource limit (5MB)
    pub const MAX_RESOURCE_STRICT: usize = 5 * MB;
    
    /// Moderate mode resource limit (50MB)
    pub const MAX_RESOURCE_MODERATE: usize = 50 * MB;
    
    /// Permissive mode resource limit (500MB)
    pub const MAX_RESOURCE_PERMISSIVE: usize = 500 * MB;
    
    /// Maximum metadata object size (100KB)
    pub const MAX_META_SIZE: usize = 100_000;
}

/// Buffer and channel sizes
pub mod buffers {
    /// Default notification channel buffer size
    pub const NOTIFICATION_BUFFER: usize = 32;
    
    /// Large notification channel buffer (for high-traffic scenarios)
    pub const NOTIFICATION_BUFFER_LARGE: usize = 1000;
    
    /// Cancellation channel buffer size
    pub const CANCELLATION_BUFFER: usize = 100;
    
    /// Duplex stream buffer size
    pub const DUPLEX_STREAM_BUFFER: usize = 1024;
}

/// Message and token limits
pub mod messages {
    /// Maximum prompt length in characters (100K)
    pub const MAX_PROMPT_LENGTH: usize = 100_000;
    
    /// Maximum tokens per turn (100K)
    pub const MAX_TOKENS_PER_TURN: usize = 100_000;
    
    /// Maximum history messages to retain
    pub const MAX_HISTORY_MESSAGES: usize = 10_000;
    
    /// Maximum content array length
    pub const MAX_CONTENT_ARRAY_LENGTH: usize = 1000;
}

/// Memory limits
pub mod memory {
    use super::content::MB;
    
    /// Maximum memory usage for base64 processing (50MB)
    pub const MAX_BASE64_MEMORY: usize = 50 * MB;
}
```

### Update Usage

```rust
// Old:
if meta_str.len() > 100_000 {
    return Err(...);
}

// New:
use crate::constants::sizes::content::MAX_META_SIZE;
if meta_str.len() > MAX_META_SIZE {
    return Err(...);
}
```

```rust
// Old:
max_resource_size: 50 * 1024 * 1024,

// New:
use crate::constants::sizes::content::MAX_RESOURCE_MODERATE;
max_resource_size: MAX_RESOURCE_MODERATE,
```

### Documentation
Add module-level docs explaining the rationale:
```rust
//! # Size Limit Constants
//! 
//! This module defines size limits organized by security level and purpose.
//! 
//! ## Security Levels
//! - **Strict**: Minimal limits for maximum security
//! - **Moderate**: Balanced limits for typical use (default)
//! - **Permissive**: Generous limits for trusted environments
//! 
//! ## Rationale
//! - 1MB strict: Prevents most DoS attacks while allowing typical content
//! - 10MB moderate: Handles images and small files comfortably
//! - 100MB permissive: Supports larger files in trusted contexts
```

## Impact
- Eliminates 50+ magic number occurrences
- Self-documenting size limits
- Consistent values across modules
- Easy to adjust limits globally
- Clear relationship between security levels
- Semantic meaning for all constants