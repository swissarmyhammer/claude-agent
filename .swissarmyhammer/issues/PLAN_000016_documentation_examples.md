# Documentation and Examples

Refer to plan.md

## Goal
Create comprehensive documentation including API docs, usage examples, integration guides, and example client implementations.

## Tasks

### 1. API Documentation with rustdoc (`lib/src/lib.rs`)

```rust
//! # Claude Agent Library
//! 
//! A Rust library that implements an Agent Client Protocol (ACP) server to wrap Claude Code functionality,
//! enabling any ACP-compatible client (Zed, Emacs, Neovim, etc.) to interact with Claude Code.
//! 
//! ## Features
//! 
//! - **Full ACP Protocol Support**: Complete implementation of the Agent Client Protocol
//! - **Streaming Responses**: Real-time streaming of Claude's responses
//! - **Tool Execution**: File system operations, terminal commands, and external tool integration
//! - **Session Management**: Multi-session support with conversation context
//! - **Security**: Comprehensive security validation and audit logging
//! - **MCP Integration**: Support for Model Context Protocol servers
//! - **Configuration**: Flexible configuration via YAML, JSON, or TOML
//! 
//! ## Quick Start
//! 
//! ```rust
//! use claude_agent_lib::{config::AgentConfig, server::ClaudeAgentServer};
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AgentConfig::default();
//!     let server = ClaudeAgentServer::new(config)?;
//!     
//!     // Start server on stdio (for use with ACP clients)
//!     server.start_stdio().await?;
//!     
//!     Ok(())
//! }
//! ```
//! 
//! ## Architecture
//! 
//! The library is structured around several key components:
//! 
//! - [`agent::ClaudeAgent`] - Core ACP agent implementation
//! - [`server::ClaudeAgentServer`] - Server infrastructure and transport handling
//! - [`session::SessionManager`] - Session lifecycle and context management
//! - [`tools::ToolCallHandler`] - Tool execution and permission management
//! - [`claude::ClaudeClient`] - Integration with Claude Code via claude-sdk-rs
//! - [`security`] - Security validation and audit logging
//! 
//! ## Configuration
//! 
//! The agent can be configured using [`config::AgentConfig`]:
//! 
//! ```rust
//! use claude_agent_lib::config::{AgentConfig, ClaudeConfig, SecurityConfig};
//! 
//! let config = AgentConfig {
//!     claude: ClaudeConfig {
//!         model: "claude-sonnet-4-20250514".to_string(),
//!         stream_format: claude_agent_lib::config::StreamFormat::StreamJson,
//!     },
//!     security: SecurityConfig {
//!         allowed_file_patterns: vec!["**/*.rs".to_string(), "**/*.md".to_string()],
//!         forbidden_paths: vec!["/etc".to_string(), "/usr".to_string()],
//!         require_permission_for: vec!["fs_write".to_string()],
//!     },
//!     ..Default::default()
//! };
//! ```

pub mod agent;
pub mod claude;
pub mod config;
pub mod error;
pub mod mcp;
pub mod server;
pub mod session;
pub mod tools;
pub mod security;

pub use error::{AgentError, Result};
```

### 2. README.md

```markdown
# Claude Agent

A Rust library and CLI tool that implements an Agent Client Protocol (ACP) server to wrap Claude Code functionality, enabling any ACP-compatible client (Zed, Emacs, Neovim, etc.) to interact with Claude Code.

## Features

- 🤖 **Full ACP Protocol Support** - Complete implementation of Agent Client Protocol v1.0.0
- ⚡ **Streaming Responses** - Real-time streaming of Claude's responses via session updates  
- 🔧 **Rich Tool Support** - File operations, terminal commands, and extensible tool system
- 🔒 **Security First** - Comprehensive validation, sandboxing, and audit logging
- 🏗️ **MCP Integration** - Support for Model Context Protocol servers
- ⚙️ **Flexible Configuration** - YAML, JSON, or TOML configuration files
- 🎯 **Multi-Session** - Concurrent session support with isolated contexts

## Quick Start

### Installation

```bash
# Install from source
git clone https://github.com/your-org/claude-agent
cd claude-agent
cargo build --release

# Or install via cargo
cargo install claude-agent-cli
```

### Usage

#### As a CLI Tool

```bash
# Start server on stdio (for ACP clients)
claude-agent serve

# Start server on specific port
claude-agent serve --port 3000

# With custom configuration
claude-agent serve --config config.yaml

# Validate configuration
claude-agent config config.yaml
```

#### As a Library

```rust
use claude_agent_lib::{config::AgentConfig, server::ClaudeAgentServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AgentConfig::default();
    let server = ClaudeAgentServer::new(config)?;
    
    server.start_stdio().await?;
    
    Ok(())
}
```

## Configuration

Create a configuration file (YAML, JSON, or TOML):

```yaml
claude:
  model: "claude-sonnet-4-20250514"
  stream_format: "StreamJson"

server:
  log_level: "info"

security:
  allowed_file_patterns: ["**/*.rs", "**/*.md", "**/*.json"]
  forbidden_paths: ["/etc", "/usr", "/bin"]
  require_permission_for: ["fs_write", "terminal_create"]

mcp_servers:
  - name: "filesystem"
    command: "npx"
    args: ["@modelcontextprotocol/server-filesystem", "--", "."]
```

## Editor Integration

### Zed

Add to your Zed settings:

```json
{
  "experimental": {
    "agent_client_protocol": {
      "command": "claude-agent",
      "args": ["serve"]
    }
  }
}
```

### Emacs

Install the ACP package and configure:

```elisp
(setq acp-server-command "claude-agent")
(setq acp-server-args '("serve"))
```

### Neovim

Using the ACP plugin:

```lua
require('acp').setup {
  server = {
    cmd = { 'claude-agent', 'serve' },
  }
}
```

## Architecture

```
┌─────────────────┐
│   ACP Client    │ (Zed, Emacs, Neovim)
│   (Editor)      │
└─────────┬───────┘
          │ JSON-RPC over stdio
          ▼
┌─────────────────┐
│ Claude Agent    │
│ ACP Server      │
├─────────────────┤
│ • Session Mgmt  │
│ • Tool Handling │
│ • Security      │
│ • Streaming     │
└─────────┬───────┘
          │
          ▼
┌─────────────────┐     ┌─────────────────┐
│   Claude Code   │ ◄── │  MCP Servers    │
│   (via SDK)     │     │ (External Tools)│
└─────────────────┘     └─────────────────┘
```

## Security

Claude Agent implements multiple security layers:

- **Path Validation** - Prevents directory traversal and unauthorized file access
- **Command Sanitization** - Blocks dangerous commands and shell injection
- **Sandboxing** - Restricts operations to allowed directories and patterns  
- **Rate Limiting** - Prevents abuse through request throttling
- **Audit Logging** - Comprehensive logging of all security-relevant events
- **Permission System** - User consent required for sensitive operations

## Development

### Prerequisites

- Rust 1.75+ 
- Claude Code API access
- Optional: Node.js for MCP servers

### Building

```bash
git clone https://github.com/your-org/claude-agent
cd claude-agent

# Build library and CLI
cargo build --release

# Run tests
cargo test --all-features

# Generate documentation
cargo doc --open
```

### Testing

```bash
# Unit tests
cargo test

# Integration tests  
cargo test --test integration

# With Claude SDK (requires API key)
CLAUDE_API_KEY=your_key cargo test --all-features
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests and documentation
5. Ensure `cargo test` and `cargo clippy` pass
6. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [Agent Client Protocol](https://github.com/agent-client-protocol/spec) specification
- [Claude SDK for Rust](https://github.com/anthropics/claude-sdk-rs)
- [Model Context Protocol](https://modelcontextprotocol.io/)
```

### 3. Usage Examples (`examples/`)

#### Basic Server Example (`examples/basic_server.rs`)

```rust
//! Basic server example demonstrating how to start a Claude Agent server.
//! 
//! This example shows the minimal setup needed to create and run a Claude Agent
//! server that can communicate with ACP clients.

use claude_agent_lib::{
    config::AgentConfig,
    server::ClaudeAgentServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();
    
    println!("Starting Claude Agent server...");
    
    // Create default configuration
    let config = AgentConfig::default();
    
    // Create server
    let server = ClaudeAgentServer::new(config)?;
    
    // Start server on stdio (standard for ACP)
    // This will run until the process is terminated
    server.start_stdio().await?;
    
    println!("Server stopped.");
    Ok(())
}
```

#### Custom Configuration Example (`examples/custom_config.rs`)

```rust
//! Example demonstrating custom configuration and MCP server integration.

use claude_agent_lib::{
    config::{AgentConfig, ClaudeConfig, SecurityConfig, McpServerConfig, StreamFormat},
    server::ClaudeAgentServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();
    
    // Create custom configuration
    let config = AgentConfig {
        claude: ClaudeConfig {
            model: "claude-sonnet-4-20250514".to_string(),
            stream_format: StreamFormat::StreamJson,
        },
        security: SecurityConfig {
            allowed_file_patterns: vec![
                "**/*.rs".to_string(),
                "**/*.md".to_string(),
                "**/*.toml".to_string(),
                "**/*.json".to_string(),
            ],
            forbidden_paths: vec![
                "/etc".to_string(),
                "/usr".to_string(),
                "/bin".to_string(),
                "/sys".to_string(),
                "/proc".to_string(),
            ],
            require_permission_for: vec![
                "fs_write".to_string(),
                "terminal_create".to_string(),
            ],
        },
        mcp_servers: vec![
            McpServerConfig {
                name: "filesystem".to_string(),
                command: "npx".to_string(),
                args: vec![
                    "@modelcontextprotocol/server-filesystem".to_string(),
                    "--".to_string(),
                    ".".to_string(),
                ],
            },
            McpServerConfig {
                name: "git".to_string(),
                command: "npx".to_string(),
                args: vec![
                    "@modelcontextprotocol/server-git".to_string(),
                    "--".to_string(),
                    ".".to_string(),
                ],
            },
        ],
        ..Default::default()
    };
    
    println!("Starting Claude Agent with custom configuration...");
    println!("Security patterns: {} allowed", config.security.allowed_file_patterns.len());
    println!("MCP servers: {}", config.mcp_servers.len());
    
    let server = ClaudeAgentServer::new(config)?;
    server.start_stdio().await?;
    
    Ok(())
}
```

#### Test Client Example (`examples/test_client.rs`)

```rust
//! Example ACP client for testing communication with Claude Agent server.
//! 
//! This demonstrates how to implement a basic ACP client that can communicate
//! with the Claude Agent server for testing and development purposes.

use agent_client_protocol::{
    InitializeRequest, AuthenticateRequest, SessionNewRequest, PromptRequest,
    ProtocolVersion, ClientCapabilities,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Command, Stdio};
use serde_json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting test ACP client...");
    
    // Start Claude Agent server as child process
    let mut server = Command::new("claude-agent")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    let mut server_stdin = server.stdin.take().unwrap();
    let server_stdout = server.stdout.take().unwrap();
    let mut server_reader = BufReader::new(server_stdout);
    
    // Initialize protocol
    println!("1. Initializing...");
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocol_version": "1.0.0",
            "client_capabilities": {
                "streaming": true,
                "tools": true
            }
        }
    });
    
    send_request(&mut server_stdin, &init_request).await?;
    let init_response = read_response(&mut server_reader).await?;
    println!("Initialize response: {}", init_response);
    
    // Authenticate
    println!("2. Authenticating...");
    let auth_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "authenticate", 
        "params": {
            "auth_type": "none"
        }
    });
    
    send_request(&mut server_stdin, &auth_request).await?;
    let auth_response = read_response(&mut server_reader).await?;
    println!("Auth response: {}", auth_response);
    
    // Create session
    println!("3. Creating session...");
    let session_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "session_new",
        "params": {
            "client_capabilities": {
                "streaming": true,
                "tools": true
            }
        }
    });
    
    send_request(&mut server_stdin, &session_request).await?;
    let session_response = read_response(&mut server_reader).await?;
    println!("Session response: {}", session_response);
    
    // Extract session ID
    let session_id = session_response["result"]["session_id"].as_str()
        .ok_or("No session ID in response")?;
    
    // Send prompt
    println!("4. Sending prompt...");
    let prompt_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "session_prompt",
        "params": {
            "session_id": session_id,
            "prompt": "Hello! Can you help me understand how this ACP server works?"
        }
    });
    
    send_request(&mut server_stdin, &prompt_request).await?;
    let prompt_response = read_response(&mut server_reader).await?;
    println!("Prompt response: {}", prompt_response);
    
    // Clean shutdown
    println!("5. Shutting down...");
    server.kill().await?;
    
    println!("Test completed successfully!");
    Ok(())
}

async fn send_request(
    writer: &mut tokio::process::ChildStdin,
    request: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let request_line = format!("{}\n", request);
    writer.write_all(request_line.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_response(
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;
    let response: serde_json::Value = serde_json::from_str(&response_line)?;
    Ok(response)
}
```

### 4. Integration Guides (`docs/integration/`)

#### Zed Editor Integration (`docs/integration/zed.md`)

```markdown
# Zed Editor Integration

This guide explains how to integrate Claude Agent with the Zed editor using the Agent Client Protocol.

## Prerequisites

- Zed editor with ACP support
- Claude Agent CLI installed
- Claude Code API access

## Installation

1. Install Claude Agent:
   ```bash
   cargo install claude-agent-cli
   ```

2. Verify installation:
   ```bash
   claude-agent info
   ```

## Configuration

### Basic Setup

Add the following to your Zed settings (`~/.config/zed/settings.json`):

```json
{
  "experimental": {
    "agent_client_protocol": {
      "command": "claude-agent",
      "args": ["serve"],
      "capabilities": {
        "streaming": true,
        "tools": true
      }
    }
  }
}
```

### Advanced Configuration

For custom behavior, create a configuration file and reference it:

```json
{
  "experimental": {
    "agent_client_protocol": {
      "command": "claude-agent",
      "args": ["serve", "--config", "/path/to/config.yaml"],
      "capabilities": {
        "streaming": true,
        "tools": true
      }
    }
  }
}
```

## Usage

Once configured, you can:

1. **Start a session**: Open command palette and select "Start Agent Session"
2. **Send prompts**: Type questions or requests in the agent panel
3. **File operations**: Ask Claude to read, write, or analyze files in your project
4. **Terminal commands**: Request execution of terminal commands (with permission)
5. **Streaming responses**: See Claude's responses appear in real-time

## Troubleshooting

### Common Issues

**Agent not starting**:
- Check that `claude-agent` is in your PATH
- Verify Claude Code API credentials
- Check logs: `claude-agent serve --log-level debug`

**Permission errors**:
- Review security configuration in your config file
- Check file patterns and forbidden paths
- Grant permissions when prompted

**Slow responses**:
- Ensure good internet connection for Claude API
- Check if MCP servers are responding
- Consider adjusting configuration for your use case
```

### 5. API Reference (`docs/api.md`)

```markdown
# API Reference

This document provides detailed API documentation for the Claude Agent library.

## Core Types

### AgentConfig

The main configuration structure for the Claude Agent.

```rust
pub struct AgentConfig {
    pub claude: ClaudeConfig,
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub mcp_servers: Vec<McpServerConfig>,
}
```

#### Fields

- `claude`: Configuration for Claude Code integration
- `server`: Server-specific settings
- `security`: Security policies and restrictions
- `mcp_servers`: List of MCP servers to connect to

### ClaudeAgent

The core ACP agent implementation.

```rust
impl Agent for ClaudeAgent {
    async fn initialize(&self, request: InitializeRequest) -> Result<InitializeResponse>;
    async fn authenticate(&self, request: AuthenticateRequest) -> Result<AuthenticateResponse>;
    async fn session_new(&self, request: SessionNewRequest) -> Result<SessionNewResponse>;
    async fn session_prompt(&self, request: PromptRequest) -> Result<PromptResponse>;
    async fn tool_permission_grant(&self, request: ToolPermissionGrantRequest) -> Result<ToolPermissionGrantResponse>;
    async fn tool_permission_deny(&self, request: ToolPermissionDenyRequest) -> Result<ToolPermissionDenyResponse>;
}
```

### ClaudeAgentServer

Server infrastructure for handling ACP communication.

```rust
impl ClaudeAgentServer {
    pub fn new(config: AgentConfig) -> Result<Self>;
    pub async fn start_stdio(&self) -> Result<()>;
    pub async fn start_with_streams<R, W>(&self, reader: R, writer: W) -> Result<()>;
    pub async fn start_with_shutdown(&self) -> Result<()>;
}
```

## Tool System

### Built-in Tools

- **fs_read**: Read file contents
- **fs_write**: Write file contents  
- **fs_list**: List directory contents
- **terminal_create**: Create terminal session
- **terminal_write**: Execute terminal command

### Tool Arguments

#### fs_read
```json
{
  "path": "/path/to/file.txt"
}
```

#### fs_write
```json
{
  "path": "/path/to/file.txt",
  "content": "File contents here"
}
```

#### terminal_write
```json
{
  "terminal_id": "terminal-uuid",
  "command": "ls -la"
}
```

## Error Types

```rust
pub enum AgentError {
    Claude(claude_sdk_rs::Error),
    Protocol(String),
    Session(String),
    ToolExecution(String),
    Config(String),
    Io(std::io::Error),
    Serialization(serde_json::Error),
    PermissionDenied(String),
    InvalidRequest(String),
    MethodNotFound(String),
    Internal(String),
}
```

## Configuration Reference

### Security Configuration

```yaml
security:
  allowed_file_patterns: 
    - "**/*.rs"
    - "**/*.md"
    - "**/*.json"
  forbidden_paths:
    - "/etc"
    - "/usr"
    - "/bin"
  require_permission_for:
    - "fs_write"
    - "terminal_create"
```

### MCP Server Configuration

```yaml
mcp_servers:
  - name: "filesystem"
    command: "npx"
    args: ["@modelcontextprotocol/server-filesystem", "--", "."]
  - name: "git"  
    command: "npx"
    args: ["@modelcontextprotocol/server-git", "--", "."]
```
```

### 6. Troubleshooting Guide (`docs/troubleshooting.md`)

```markdown
# Troubleshooting Guide

This guide helps resolve common issues when using Claude Agent.

## Installation Issues

### Cargo Build Failures

**Problem**: Build fails with dependency errors
```
error: failed to compile claude-agent
```

**Solutions**:
1. Update Rust: `rustup update`
2. Clear cargo cache: `cargo clean`
3. Check Rust version: `rustc --version` (requires 1.75+)

### Missing Dependencies

**Problem**: Runtime errors about missing commands
```
Error: MCP server command not found: npx
```

**Solutions**:
1. Install Node.js for MCP servers: `npm install -g @modelcontextprotocol/server-*`
2. Check PATH includes required commands
3. Disable MCP servers in configuration if not needed

## Runtime Issues

### Authentication Failures

**Problem**: Claude API authentication fails
```
Error: Claude SDK error: Invalid API key
```

**Solutions**:
1. Set environment variable: `export CLAUDE_API_KEY=your_key`
2. Check API key validity at Claude dashboard
3. Verify network connectivity to Claude API

### Permission Denied

**Problem**: File or command operations fail
```
Error: Permission denied: Access forbidden to path
```

**Solutions**:
1. Check `allowed_file_patterns` in configuration
2. Remove paths from `forbidden_paths` if needed
3. Grant permission when prompted in editor
4. Run with appropriate file system permissions

### High Memory Usage

**Problem**: Agent consumes excessive memory
```
Process killed due to memory usage
```

**Solutions**:
1. Limit session count and cleanup old sessions
2. Reduce MCP server count
3. Check for memory leaks in logs
4. Restart agent periodically

## Protocol Issues

### Connection Refused

**Problem**: Editor cannot connect to agent
```
Error: Connection refused on stdio
```

**Solutions**:
1. Verify agent starts successfully: `claude-agent serve --log-level debug`
2. Check editor ACP configuration
3. Ensure no other processes using stdio
4. Try port-based connection instead

### Streaming Not Working

**Problem**: Responses appear all at once instead of streaming
```
No streaming updates received
```

**Solutions**:
1. Enable streaming in client capabilities
2. Check network stability
3. Verify Claude model supports streaming
4. Check for proxy interference

### Tool Calls Failing

**Problem**: Tool execution fails unexpectedly
```
Error: Tool execution error: Unknown tool
```

**Solutions**:
1. Check tool name spelling
2. Verify tool is in capabilities list
3. Check security configuration allows tool
4. Review audit logs for details

## Performance Issues

### Slow Response Times

**Problem**: Agent responses are very slow
```
Request timeout after 30 seconds
```

**Solutions**:
1. Check internet connection to Claude API
2. Reduce prompt complexity
3. Optimize MCP server performance
4. Increase timeout values if needed

### High CPU Usage

**Problem**: Agent process uses excessive CPU
```
Agent process at 100% CPU
```

**Solutions**:
1. Check for infinite loops in tool execution
2. Reduce concurrent session count
3. Optimize regular expressions in security validation
4. Profile with debugging tools

## Debugging

### Enable Debug Logging

```bash
# CLI
claude-agent serve --log-level debug

# Environment variable
RUST_LOG=debug claude-agent serve

# Configuration file
server:
  log_level: "debug"
```

### Check Audit Logs

```bash
# View audit log
tail -f audit.log

# Search for specific session
grep "session-id" audit.log

# Check security violations
grep "SecurityViolation" audit.log
```

### Validate Configuration

```bash
# Test configuration file
claude-agent config config.yaml

# Show current configuration
claude-agent info
```

## Getting Help

1. **Check logs**: Always check debug logs first
2. **Validate configuration**: Use `claude-agent config` command
3. **Test isolation**: Try minimal configuration
4. **Check network**: Verify Claude API connectivity
5. **Review security**: Check if security policies block operation

If issues persist:
- Open GitHub issue with logs and configuration
- Include system information and versions
- Provide minimal reproduction steps
```

### 7. Examples Integration in Cargo.toml

Add to root `Cargo.toml`:

```toml
[[example]]
name = "basic_server"
path = "examples/basic_server.rs"

[[example]]
name = "custom_config"
path = "examples/custom_config.rs"

[[example]]
name = "test_client"
path = "examples/test_client.rs"
```

### 8. GitHub Actions for Documentation

`.github/workflows/docs.yml`:

```yaml
name: Documentation

on:
  push:
    branches: [ main ]
  pull_request:
    branches: [ main ]

jobs:
  docs:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
      
    - name: Build documentation
      run: |
        cargo doc --all-features --no-deps
        
    - name: Deploy to GitHub Pages
      if: github.ref == 'refs/heads/main'
      uses: peaceiris/actions-gh-pages@v3
      with:
        github_token: ${{ secrets.GITHUB_TOKEN }}
        publish_dir: ./target/doc
```

## Files Created
- `lib/src/lib.rs` - Enhanced with comprehensive rustdoc documentation
- `README.md` - Complete project documentation with usage examples
- `examples/basic_server.rs` - Basic server usage example
- `examples/custom_config.rs` - Advanced configuration example  
- `examples/test_client.rs` - ACP client testing example
- `docs/integration/zed.md` - Zed editor integration guide
- `docs/api.md` - Complete API reference documentation
- `docs/troubleshooting.md` - Comprehensive troubleshooting guide
- `.github/workflows/docs.yml` - Documentation CI/CD

## Dependencies
Add to `Cargo.toml`:
```toml
[dev-dependencies]
# ... existing dev dependencies ...

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

## Acceptance Criteria
- Comprehensive API documentation with rustdoc
- Clear README with quick start and examples
- Working code examples for basic and advanced usage
- Integration guides for popular editors (Zed, Emacs, Neovim)
- Complete API reference covering all public types and methods
- Troubleshooting guide covering common issues and solutions
- Example configurations for different use cases
- Documentation builds successfully with `cargo doc`
- Examples run successfully with `cargo run --example`
- GitHub Actions publishes documentation automatically
- All documentation is accurate and up-to-date with implementation