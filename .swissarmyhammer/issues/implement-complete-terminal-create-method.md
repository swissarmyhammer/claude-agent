# Implement Complete terminal/create Method

## Problem
Our terminal creation implementation may not support all parameters required by the ACP specification. We need complete support for the `terminal/create` method including command arguments, environment variables, working directory, and output byte limits.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/terminals:

**Complete Method Signature:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "terminal/create",
  "params": {
    "sessionId": "sess_abc123def456",
    "command": "npm",
    "args": ["test", "--coverage"],
    "env": [
      {"name": "NODE_ENV", "value": "test"},
      {"name": "DEBUG", "value": "true"}
    ],
    "cwd": "/home/user/project",
    "outputByteLimit": 1048576
  }
}
```

**Response Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "terminalId": "term_xyz789"
  }
}
```

## Current Issues
- Terminal creation implementation completeness unclear
- Environment variable support may not be implemented
- Output byte limit enforcement unclear
- Terminal ID generation and management unclear

## Implementation Tasks

### Method Handler Implementation
- [ ] Implement complete `terminal/create` method handler
- [ ] Add proper JSON-RPC method registration
- [ ] Support all required and optional parameters
- [ ] Generate unique terminal IDs for tracking

### Parameter Support
- [ ] Support required `sessionId` parameter with validation
- [ ] Support required `command` parameter
- [ ] Support optional `args` array parameter
- [ ] Support optional `env` array parameter with name/value pairs
- [ ] Support optional `cwd` parameter (absolute path)
- [ ] Support optional `outputByteLimit` parameter

### Terminal ID Generation and Management
- [ ] Generate unique terminal IDs (e.g., `term_` prefix + unique suffix)
- [ ] Maintain terminal registry for tracking active terminals
- [ ] Add terminal ID validation and format consistency
- [ ] Handle terminal ID conflicts and collision detection

### Process Creation and Management
- [ ] Create child processes with specified command and arguments
- [ ] Set environment variables for child processes
- [ ] Set working directory for child processes
- [ ] Handle process creation errors and validation

## Terminal Creation Implementation
```rust
#[derive(Debug, Deserialize)]
pub struct TerminalCreateParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<Vec<EnvVariable>>,
    pub cwd: Option<String>,
    #[serde(rename = "outputByteLimit")]
    pub output_byte_limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct EnvVariable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct TerminalCreateResponse {
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

pub async fn handle_terminal_create(
    params: TerminalCreateParams
) -> Result<TerminalCreateResponse, TerminalError> {
    // Validate session ID
    validate_session_id(&params.session_id)?;
    
    // Validate and prepare command
    let command_config = CommandConfig {
        command: params.command,
        args: params.args.unwrap_or_default(),
        env: params.env.unwrap_or_default(),
        cwd: params.cwd,
        output_byte_limit: params.output_byte_limit.unwrap_or(1048576), // 1MB default
    };
    
    // Create and start terminal
    let terminal_id = generate_terminal_id();
    let terminal = Terminal::create(terminal_id.clone(), command_config).await?;
    
    // Register terminal in active registry
    TERMINAL_REGISTRY.register(terminal_id.clone(), terminal).await;
    
    Ok(TerminalCreateResponse { terminal_id })
}

fn generate_terminal_id() -> String {
    format!("term_{}", ulid::Ulid::new())
}
```

## Implementation Notes
Add terminal creation comments:
```rust
// ACP terminal/create method implementation:
// 1. sessionId: Required - validate against active sessions
// 2. command: Required - command to execute
// 3. args: Optional - command arguments array
// 4. env: Optional - environment variables with name/value pairs
// 5. cwd: Optional - working directory (absolute path)
// 6. outputByteLimit: Optional - output buffer size limit
// 7. Response: terminalId for subsequent operations
//
// Creates background process with real-time output capture.
```

### Environment Variable Handling
```rust
impl CommandConfig {
    pub fn apply_environment_variables(&self) -> std::collections::HashMap<String, String> {
        let mut env_vars = std::env::vars().collect::<std::collections::HashMap<_, _>>();
        
        // Apply custom environment variables
        for env_var in &self.env {
            env_vars.insert(env_var.name.clone(), env_var.value.clone());
        }
        
        env_vars
    }
}
```

### Working Directory Management
- [ ] Use session working directory as default if `cwd` not specified
- [ ] Validate `cwd` parameter is absolute path
- [ ] Apply working directory to child process
- [ ] Handle working directory access errors

### Output Buffer Management
- [ ] Implement output byte limit enforcement
- [ ] Add output buffer truncation from beginning when limit exceeded
- [ ] Ensure truncation happens at character boundaries
- [ ] Track truncation status for reporting

### Process Creation Error Handling
- [ ] Handle command not found errors
- [ ] Handle permission denied errors for command execution
- [ ] Handle working directory access errors
- [ ] Handle environment variable validation errors

## Testing Requirements
- [ ] Test terminal creation with all parameter combinations
- [ ] Test terminal ID generation uniqueness
- [ ] Test command execution with arguments and environment variables
- [ ] Test working directory application and validation
- [ ] Test output byte limit enforcement and truncation
- [ ] Test error scenarios (command not found, permissions, etc.)
- [ ] Test concurrent terminal creation
- [ ] Test terminal registry management

## Integration Points
- [ ] Connect to session validation and management
- [ ] Integrate with process creation and management systems
- [ ] Connect to output capture and streaming systems
- [ ] Integrate with terminal lifecycle management

## Security Considerations
- [ ] Validate commands against security policies
- [ ] Sanitize environment variables for security
- [ ] Validate working directory boundaries
- [ ] Implement command execution sandboxing where appropriate

## Acceptance Criteria
- Complete `terminal/create` method handler with all parameters
- Unique terminal ID generation and registry management
- Environment variable support with name/value pairs
- Working directory integration with session context
- Output byte limit enforcement with character boundary truncation
- Proper error handling for all failure scenarios
- Integration with session validation and capability checking
- Security validation for command execution and parameters
- Comprehensive test coverage for all creation scenarios
- Documentation of method behavior and requirements