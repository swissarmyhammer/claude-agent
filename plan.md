# Claude Agent ACP Implementation Plan

## Overview

Create a Rust library and CLI tool to implement an Agent Client Protocol (ACP) server that wraps Claude Code functionality, enabling any ACP-compatible client (like Zed, Emacs, Neovim) to interact with Claude Code.

## Architecture

### Core Components

1. **Library (`claude-agent-lib`)**: Core ACP server implementation
2. **CLI (`claude-agent-cli`)**: Simple command-line interface to start the server
3. **Integration Layer**: Bridge between ACP and Claude Code via claude-sdk-rs

### Technology Stack

- **Language**: Rust (for performance, safety, and ecosystem alignment)
- **ACP Protocol**: `agent-client-protocol` crate (v0.4.3)
- **Claude Integration**: `claude-sdk-rs` crate (v1.0.1) 
- **Transport**: JSON-RPC over stdio (standard ACP pattern)
- **Logging**: `tracing` and `tracing-subscriber`
- **Async Runtime**: `tokio`

## Implementation Plan

### Phase 1: Project Setup & Foundation

#### 1.1 Project Structure
```
claude-agent/
├── Cargo.toml (workspace)
├── README.md
├── LICENSE
├── .gitignore
├── lib/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── agent.rs          # ACP Agent trait implementation
│   │   ├── claude.rs         # Claude SDK wrapper
│   │   ├── server.rs         # ACP server management
│   │   ├── session.rs        # Session management
│   │   ├── tools.rs          # Tool call handling
│   │   ├── config.rs         # Configuration types
│   │   └── error.rs          # Error types
│   └── tests/
└── cli/
    ├── Cargo.toml
    ├── src/
    │   └── main.rs           # CLI entry point
    └── tests/
```

#### 1.2 Dependencies Setup
**Library (`lib/Cargo.toml`):**
```toml
[dependencies]
agent-client-protocol = "0.4.3"
claude-sdk-rs = { version = "1.0.1", features = ["full"] }
tokio = { version = "1.40", features = ["macros", "rt", "io-std", "process"] }
tokio-util = { version = "0.7", features = ["compat"] }
log = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
async-trait = "0.1"
futures = "0.3"

[dev-dependencies]
env_logger = "0.11"
tokio-test = "0.4"
```

**CLI (`cli/Cargo.toml`):**
```toml
[dependencies]
claude-agent-lib = { path = "../lib" }
clap = { version = "4.5", features = ["derive"] }
env_logger = "0.11"
tokio = { version = "1.40", features = ["macros", "rt"] }
anyhow = "1.0"
```

### Phase 2: Core Agent Implementation

#### 2.1 Agent Trait Implementation (`lib/src/agent.rs`)

```rust
use tokio::sync::{mpsc, oneshot};
use std::cell::Cell;

pub struct ClaudeAgent {
    claude_client: ClaudeClient,
    session_manager: SessionManager,
    session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>,
    next_session_id: Cell<u32>,
}

impl ClaudeAgent {
    pub fn new(
        config: AgentConfig, 
        session_update_tx: mpsc::UnboundedSender<(acp::SessionNotification, oneshot::Sender<()>)>
    ) -> Self {
        Self {
            claude_client: ClaudeClient::new(config.claude),
            session_manager: SessionManager::new(),
            session_update_tx,
            next_session_id: Cell::new(0),
        }
    }
    
    async fn send_session_update(&self, update: acp::SessionUpdate, session_id: &acp::SessionId) -> Result<(), acp::Error> {
        let (ack_tx, ack_rx) = oneshot::channel();
        let notification = acp::SessionNotification {
            session_id: session_id.clone(),
            update,
            meta: None,
        };
        
        self.session_update_tx
            .send((notification, ack_tx))
            .map_err(|_| acp::Error::internal_error())?;
            
        ack_rx.await.map_err(|_| acp::Error::internal_error())?;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Agent for ClaudeAgent {
    async fn initialize(&self, args: acp::InitializeRequest) -> Result<acp::InitializeResponse, acp::Error> {
        Ok(acp::InitializeResponse {
            protocol_version: acp::V1,
            agent_capabilities: acp::AgentCapabilities {
                // Configure based on Claude Code capabilities
                prompt: Some(acp::PromptCapabilities::default()),
                mcp: Some(acp::McpCapabilities::default()),
                ..Default::default()
            },
            auth_methods: Vec::new(),
            meta: None,
        })
    }
    
    async fn new_session(&self, args: acp::NewSessionRequest) -> Result<acp::NewSessionResponse, acp::Error> {
        let session_id = self.next_session_id.get();
        self.next_session_id.set(session_id + 1);
        let session_id = acp::SessionId(session_id.to_string().into());
        
        self.session_manager.create_session(&session_id, args.mcp_servers).await?;
        
        Ok(acp::NewSessionResponse {
            session_id,
            modes: None,
            meta: None,
        })
    }
    
    async fn prompt(&self, args: acp::PromptRequest) -> Result<acp::PromptResponse, acp::Error> {
        // Convert ACP prompt to Claude SDK format
        let prompt = self.format_prompt(args.prompt);
        
        // Stream response from Claude Code
        let mut stream = self.claude_client.query_stream(&prompt, &args.session_id).await
            .map_err(|e| acp::Error::internal_error_with_message(e.to_string()))?;
            
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(content) => {
                    self.send_session_update(
                        acp::SessionUpdate::AgentMessageChunk { content: content.into() },
                        &args.session_id
                    ).await?;
                }
                Err(e) => return Err(acp::Error::internal_error_with_message(e.to_string())),
            }
        }
        
        Ok(acp::PromptResponse {
            stop_reason: acp::StopReason::EndTurn,
            meta: None,
        })
    }
    
    async fn cancel(&self, args: acp::CancelNotification) -> Result<(), acp::Error> {
        // Cancel Claude Code operations for session
        self.claude_client.cancel_session(&args.session_id).await
            .map_err(|e| acp::Error::internal_error_with_message(e.to_string()))?;
        Ok(())
    }
    
    // Implement remaining methods with appropriate delegation to Claude Code
    async fn authenticate(&self, _args: acp::AuthenticateRequest) -> Result<acp::AuthenticateResponse, acp::Error> {
        Ok(acp::AuthenticateResponse::default())
    }
    
    // ... other trait methods
}
```

**Key Implementation Details:**
- **Session Notifications**: Uses mpsc channel with oneshot acknowledgment
- **Streaming**: Forwards Claude Code stream chunks as ACP session updates  
- **Error Mapping**: Converts Claude SDK errors to ACP protocol errors
- **Session Management**: Tracks sessions and their MCP server configurations

#### 2.2 Claude Integration Layer (`lib/src/claude.rs`)

Wrapper around `claude-sdk-rs` to provide:

```rust
pub struct ClaudeClient {
    client: claude_sdk_rs::Client,
    config: claude_sdk_rs::Config,
}

impl ClaudeClient {
    pub fn new() -> Result<Self, Error>;
    pub async fn query_stream(&self, prompt: &str, session_id: &str) -> Result<MessageStream, Error>;
    pub async fn query(&self, prompt: &str, session_id: &str) -> Result<String, Error>;
    pub fn supports_streaming(&self) -> bool;
}
```

**Configuration:**
- Use JSON streaming mode for real-time updates
- Session persistence for conversation context
- Tool call integration
- Error handling and retry logic

#### 2.3 Session Management (`lib/src/session.rs`)

```rust
pub struct SessionManager {
    sessions: HashMap<SessionId, Session>,
}

pub struct Session {
    id: SessionId,
    created_at: SystemTime,
    context: Vec<Message>,
    client_capabilities: ClientCapabilities,
    mcp_servers: Vec<McpServer>,
}
```

**Features:**
- Thread-safe session storage
- Session cleanup and expiration
- Context management for multi-turn conversations
- MCP server configuration per session

### Phase 3: Protocol Integration

#### 3.1 ACP Server Setup (`lib/src/server.rs`)

```rust
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

pub struct ClaudeAgentServer {
    config: AgentConfig,
}

impl ClaudeAgentServer {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }
    
    pub async fn start_stdio(&self) -> Result<(), Error> {
        let outgoing = tokio::io::stdout().compat_write();
        let incoming = tokio::io::stdin().compat();
        self.start_with_streams(incoming, outgoing).await
    }
    
    pub async fn start_with_streams<R, W>(&self, reader: R, writer: W) -> Result<(), Error>
    where
        R: AsyncRead + Unpin + 'static,
        W: AsyncWrite + Unpin + 'static,
    {
        let local_set = tokio::task::LocalSet::new();
        local_set.run_until(async move {
            let (session_tx, mut session_rx) = mpsc::unbounded_channel();
            let agent = ClaudeAgent::new(self.config.clone(), session_tx);
            
            // Create ACP connection - futures are NOT Send!
            let (conn, handle_io) = acp::AgentSideConnection::new(
                agent, 
                writer, 
                reader, 
                |fut| { tokio::task::spawn_local(fut); }
            );
            
            // Background task for session notifications
            tokio::task::spawn_local(async move {
                while let Some((notification, ack_tx)) = session_rx.recv().await {
                    if let Err(e) = conn.session_notification(notification).await {
                        log::error!("Session notification failed: {e}");
                        break;
                    }
                    ack_tx.send(()).ok();
                }
            });
            
            handle_io.await
        }).await
    }
}
```

**Key Implementation Details:**
- **LocalSet Required**: ACP futures are not Send, must use `spawn_local`
- **Compatibility Layer**: Use `tokio_util::compat` for AsyncRead/Write
- **Session Notifications**: Background task with mpsc channel for streaming updates
- **Generic Streams**: Testable with in-memory streams, production uses stdio

#### 3.2 Tool Call Handling (`lib/src/tools.rs`)

Bridge Claude Code's tool system with ACP's permission model:

```rust
pub struct ToolCallHandler {
    client_connection: Arc<dyn Client>,
}

impl ToolCallHandler {
    pub async fn handle_tool_call(&self, tool_call: ToolCall) -> Result<ToolCallContent, Error>;
    pub async fn request_permission(&self, tool_call: &ToolCall) -> Result<bool, Error>;
}
```

**Supported Tool Categories:**
- File system operations (read, write, create, delete)
- Terminal/shell command execution
- Code analysis and refactoring
- Git operations
- Project-specific tools via MCP servers

### Phase 4: CLI Implementation

#### 4.1 Minimal CLI (`cli/src/main.rs`)

```rust
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Port to bind the ACP server (optional, uses stdio by default)
    #[arg(short, long)]
    port: Option<u16>,
    
    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,
    

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(cli.log_level)
        .init();
    
    // Create and start server
    let server = if let Some(port) = cli.port {
        ClaudeAgentServer::with_port(port)
    } else {
        ClaudeAgentServer::new()
    };
    
    server.start().await?;
    Ok(())
}
```

**Key Implementation Details:**
- **Single-threaded runtime**: `current_thread` like ACP examples
- **env_logger**: Consistent with ACP examples (not tracing)
- **Simple stdio mode**: No TCP support, pure ACP compliance
- **Default config**: Uses programmatic configuration object
- Configuration file support (optional)

### Phase 5: Advanced Features

#### 5.1 Streaming & Real-time Updates

Leverage Claude Code's JSON streaming mode:

```rust
impl Agent for ClaudeAgent {
    async fn session_prompt(&self, request: PromptRequest) -> Result<PromptResponse, Error> {
        let mut stream = self.claude_client.query_stream(&request.prompt, &request.session_id).await?;
        
        while let Some(chunk) = stream.next().await {
            // Send real-time updates via session/update notifications
            self.send_session_update(SessionUpdate::MessageChunk {
                role: Role::Agent,
                content: chunk.content,
                session_id: request.session_id.clone(),
            }).await?;
        }
        
        Ok(PromptResponse {
            stop_reason: StopReason::EndTurn,
            session_id: request.session_id,
        })
    }
}
```

#### 5.2 MCP Server Integration

Support for user-configured MCP servers:

```rust
pub struct McpServerManager {
    servers: Vec<McpServerConnection>,
}

impl McpServerManager {
    pub async fn connect_servers(&mut self, configs: Vec<McpServer>) -> Result<(), Error>;
    pub async fn forward_tool_calls(&self, tool_calls: Vec<ToolCall>) -> Result<Vec<ToolCallContent>, Error>;
}
```



## Testing Strategy

### Unit Tests
- Agent trait implementation
- Claude client wrapper
- Session management
- Tool call handling

### Integration Tests
- End-to-end ACP protocol compliance using in-memory streams
- Claude Code integration
- Error handling and recovery

### Acceptance Tests
```rust
// tests/acceptance.rs
use agent_client_protocol as acp;
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

#[tokio::test]
async fn test_full_acp_flow() {
    // Create in-memory bidirectional streams
    let (client_writer, server_reader) = piper::pipe();
    let (server_writer, client_reader) = piper::pipe();
    
    let local_set = tokio::task::LocalSet::new();
    local_set.run_until(async move {
        // Start our server
        let config = AgentConfig::default();
        let server = ClaudeAgentServer::new(config);
        tokio::task::spawn_local(server.start_with_streams(
            server_reader.compat(),
            server_writer.compat_write()
        ));
        
        // Create test client
        let client = TestClient {};
        let (client_conn, client_handle) = acp::ClientSideConnection::new(
            client,
            client_writer.compat_write(), 
            client_reader.compat(),
            |fut| { tokio::task::spawn_local(fut); }
        );
        
        tokio::task::spawn_local(client_handle);
        
        // Test protocol flow
        client_conn.initialize(acp::InitializeRequest {
            protocol_version: acp::V1,
            client_capabilities: acp::ClientCapabilities::default(),
            meta: None,
        }).await.unwrap();
        
        let session = client_conn.new_session(acp::NewSessionRequest {
            mcp_servers: Vec::new(),
            cwd: std::env::current_dir().unwrap(),
            meta: None,
        }).await.unwrap();
        
        client_conn.prompt(acp::PromptRequest {
            session_id: session.session_id,
            prompt: vec!["Hello".into()],
            meta: None,
        }).await.unwrap();
        
    }).await;
}
```

**Test Coverage:**
- Complete ACP protocol flow using in-memory streams
- Session creation and management
- Streaming response handling
- Error conditions and edge cases

### Example Clients
- Simple test client for validation
- Integration examples for popular editors

## Deployment & Distribution

### Package Structure
- **Library**: `claude-agent-lib` crate for embedding
- **Binary**: `claude-agent-cli` for standalone use
- **Documentation**: Comprehensive API docs and usage examples







## Configuration

### Configuration Object (`lib/src/config.rs`)

```rust
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub claude: ClaudeConfig,
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub mcp_servers: Vec<McpServerConfig>,
}

#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub model: String,
    pub stream_format: StreamFormat,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    // Server-specific settings if needed
}

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub allowed_file_patterns: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub require_permission_for: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            claude: ClaudeConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                stream_format: StreamFormat::StreamJson,
            },
            server: ServerConfig {},
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
            mcp_servers: vec![
                McpServerConfig {
                    name: "filesystem".to_string(),
                    command: "mcp-server-filesystem".to_string(),
                    args: vec!["--root".to_string(), ".".to_string()],
                },
                McpServerConfig {
                    name: "git".to_string(),
                    command: "mcp-server-git".to_string(),
                    args: vec![],
                },
            ],
        }
    }
}
```

## Success Metrics

1. **Protocol Compliance**: Full ACP specification implementation
2. **Compatibility**: Working integration with major ACP clients
3. **Reliability**: 99%+ uptime with graceful error handling



