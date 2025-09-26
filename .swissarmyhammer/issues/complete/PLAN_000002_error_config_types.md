# Error and Configuration Types

Refer to plan.md

## Goal
Create foundational error types and configuration structures for the Claude Agent.

## Tasks

### 1. Error Types (`lib/src/error.rs`)
```rust
#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("Claude SDK error: {0}")]
    Claude(#[from] claude_sdk_rs::Error),
    
    #[error("Protocol error: {0}")]  
    Protocol(String),
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Tool execution error: {0}")]
    ToolExecution(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AgentError>;
```

### 2. Configuration Types (`lib/src/config.rs`)

#### AgentConfig Structure
```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AgentConfig {
    pub claude: ClaudeConfig,
    pub server: ServerConfig, 
    pub security: SecurityConfig,
    pub mcp_servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ClaudeConfig {
    pub model: String,
    pub stream_format: StreamFormat,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]  
pub struct ServerConfig {
    pub port: Option<u16>,
    pub log_level: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SecurityConfig {
    pub allowed_file_patterns: Vec<String>,
    pub forbidden_paths: Vec<String>, 
    pub require_permission_for: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum StreamFormat {
    StreamJson,
    Standard,
}
```

#### Default Implementation
```rust
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            claude: ClaudeConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                stream_format: StreamFormat::StreamJson,
            },
            server: ServerConfig {
                port: None,
                log_level: "info".to_string(),
            },
            security: SecurityConfig {
                allowed_file_patterns: vec![
                    "**/*.rs".to_string(),
                    "**/*.md".to_string(), 
                    "**/*.toml".to_string(),
                ],
                forbidden_paths: vec![
                    "/etc".to_string(),
                    "/usr".to_string(),
                    "/bin".to_string(),
                ],
                require_permission_for: vec![
                    "fs_write".to_string(),
                    "terminal_create".to_string(),
                ],
            },
            mcp_servers: vec![],
        }
    }
}
```

### 3. Configuration Validation
- Add validation methods for file patterns
- Validate path security
- Ensure MCP server configurations are valid

### 4. Unit Tests
- Test default configuration creation
- Test serialization/deserialization 
- Test validation logic
- Test error type conversions

## Files Created
- `lib/src/error.rs` - Error types and Result alias
- `lib/src/config.rs` - Configuration structures and defaults
- Update `lib/src/lib.rs` to export these modules

## Acceptance Criteria
- All error types compile and convert properly
- Configuration can be serialized to/from JSON
- Default configuration is valid
- Unit tests pass for all functionality
- `cargo build` and `cargo test` succeed

## Proposed Solution

Based on the issue requirements, I implemented a comprehensive error handling and configuration system with the following approach:

1. **Error Types (`lib/src/error.rs`)**
   - Created `AgentError` enum using `thiserror` for structured error handling
   - Implemented automatic conversions from standard library errors (`io::Error`, `serde_json::Error`)
   - Added placeholder for `claude_sdk_rs::Error` conversion (will work once SDK is integrated)
   - Created `Result<T>` type alias for convenience
   - Comprehensive unit tests covering error display, conversions, and type alias functionality

2. **Configuration Types (`lib/src/config.rs`)**
   - Implemented complete configuration hierarchy with serde support
   - Added validation methods to ensure configuration integrity
   - Implemented JSON serialization/deserialization with proper error handling
   - Created sensible defaults for all configuration options
   - Extensive unit tests covering validation, serialization, and defaults

3. **Module Integration**
   - Updated `lib/src/lib.rs` to export new modules and replace old config structures
   - Fixed CLI integration to use new configuration field names
   - Preserved all existing functionality while upgrading to typed error handling

## Implementation Notes

- **Dependency Usage**: Leveraged existing `thiserror` and `serde` dependencies from Cargo.toml
- **Backward Compatibility**: Updated existing code (CLI) to use new config structure seamlessly
- **Error Handling**: Replaced `anyhow::Result` with typed `AgentError::Result` for better error semantics
- **Configuration Validation**: Added comprehensive validation for model names, log levels, and MCP server configs
- **Security Considerations**: Default security config includes sensible file patterns and forbidden paths
- **Testing**: Created 18 unit tests covering all functionality - all tests passing

## Files Created/Modified
- ✅ `lib/src/error.rs` - Complete error type system with tests
- ✅ `lib/src/config.rs` - Complete configuration system with validation and tests  
- ✅ Updated `lib/src/lib.rs` - Module exports and integration
- ✅ Updated `cli/src/main.rs` - Fixed to use new config structure

## Validation Results
- ✅ `cargo build` - Successful compilation
- ✅ `cargo nextest run` - All 18 tests passing
- ✅ Error type conversions working properly
- ✅ Configuration serialization/deserialization functional
- ✅ Default configuration valid and tested
- ✅ CLI integration working with new config structure