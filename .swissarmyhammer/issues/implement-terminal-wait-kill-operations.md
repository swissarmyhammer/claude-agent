# Implement Terminal Wait and Kill Operations

## Problem
Our terminal implementation may not support the `terminal/wait_for_exit` and `terminal/kill` methods required by the ACP specification. These methods are essential for process lifecycle management and timeout implementation.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/terminals:

**terminal/wait_for_exit Method:**
```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "terminal/wait_for_exit",
  "params": {
    "sessionId": "sess_abc123def456",
    "terminalId": "term_xyz789"
  }
}
```

**Wait Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "result": {
    "exitCode": 0,
    "signal": null
  }
}
```

**terminal/kill Method:**
```json
{
  "jsonrpc": "2.0",
  "id": 8,
  "method": "terminal/kill",
  "params": {
    "sessionId": "sess_abc123def456",
    "terminalId": "term_xyz789"
  }
}
```

**Timeout Pattern:**
1. Create terminal with `terminal/create`
2. Start timer for timeout duration
3. Concurrently wait for timer or `terminal/wait_for_exit`
4. If timeout: call `terminal/kill`, get output, include in response
5. Always call `terminal/release` when done

## Current Issues
- `terminal/wait_for_exit` blocking wait implementation unclear
- `terminal/kill` command termination implementation unclear
- Timeout pattern implementation for command execution unclear
- Process signal handling and termination unclear

## Implementation Tasks

### Wait for Exit Implementation
- [ ] Implement `terminal/wait_for_exit` method handler
- [ ] Add blocking wait for process completion
- [ ] Return exit code and signal information
- [ ] Handle concurrent wait requests for same terminal

### Process Kill Implementation
- [ ] Implement `terminal/kill` method handler
- [ ] Add process termination with appropriate signals
- [ ] Handle process kill failures and zombie processes
- [ ] Support graceful vs forceful termination

### Exit Status Data Structure
- [ ] Define `TerminalExitStatus` with exitCode and signal fields
- [ ] Support null values for exit code (signal termination) and signal (normal exit)
- [ ] Add exit status validation and consistency checking
- [ ] Handle platform-specific exit status differences

### Timeout Pattern Implementation
- [ ] Create timeout utilities for terminal operations
- [ ] Implement concurrent wait with timeout using tokio::select!
- [ ] Add automatic kill and cleanup on timeout
- [ ] Support configurable timeout durations

## Method Implementation
```rust
#[derive(Debug, Deserialize)]
pub struct TerminalWaitParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

#[derive(Debug, Serialize)]
pub struct TerminalWaitResponse {
    #[serde(rename = "exitCode")]
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
}

pub async fn handle_terminal_wait_for_exit(
    params: TerminalWaitParams
) -> Result<TerminalWaitResponse, TerminalError> {
    // Validate session and terminal
    validate_session_id(&params.session_id)?;
    let terminal = get_terminal(&params.terminal_id)?;
    
    // Wait for process completion
    let exit_status = terminal.wait_for_exit().await?;
    
    Ok(TerminalWaitResponse {
        exit_code: exit_status.exit_code,
        signal: exit_status.signal,
    })
}

pub async fn handle_terminal_kill(
    params: TerminalKillParams
) -> Result<serde_json::Value, TerminalError> {
    // Validate session and terminal
    validate_session_id(&params.session_id)?;
    let terminal = get_terminal(&params.terminal_id)?;
    
    // Kill the process
    terminal.kill().await?;
    
    // Return null result per ACP specification
    Ok(serde_json::Value::Null)
}
```

## Implementation Notes
Add terminal wait and kill comments:
```rust
// ACP terminal process control implementation:
// 1. terminal/wait_for_exit: Blocking wait for process completion
// 2. terminal/kill: Terminate process without releasing terminal
// 3. Exit status: Report exitCode for normal exit, signal for termination
// 4. Timeout pattern: Combine wait and kill for command timeouts
// 5. Resource management: Terminal remains valid after kill until release
//
// Process control enables timeout implementation and resource management.
```

### Signal Handling Implementation
```rust
impl Terminal {
    pub async fn kill(&self) -> Result<(), TerminalError> {
        let mut process = self.process.lock().await;
        
        #[cfg(unix)]
        {
            // Try graceful termination first (SIGTERM)
            process.kill().await
                .map_err(|e| TerminalError::KillFailed(e))?;
            
            // Wait briefly for graceful shutdown
            match tokio::time::timeout(
                Duration::from_secs(5),
                process.wait()
            ).await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    // Force kill if graceful shutdown fails
                    unsafe {
                        libc::kill(process.id() as i32, libc::SIGKILL);
                    }
                }
            }
        }
        
        #[cfg(windows)]
        {
            process.kill().await
                .map_err(|e| TerminalError::KillFailed(e))?;
        }
        
        Ok(())
    }
    
    pub async fn wait_for_exit(&self) -> Result<TerminalExitStatus, TerminalError> {
        let exit_status = self.process.wait().await
            .map_err(|e| TerminalError::WaitFailed(e))?;
        
        Ok(TerminalExitStatus {
            exit_code: exit_status.code(),
            signal: self.get_termination_signal(&exit_status),
        })
    }
}
```

### Timeout Pattern Utilities
```rust
pub struct TerminalTimeout {
    terminal_id: String,
    timeout_duration: Duration,
}

impl TerminalTimeout {
    pub async fn execute_with_timeout(&self) -> Result<TerminalExitStatus, TerminalError> {
        let terminal = get_terminal(&self.terminal_id)?;
        
        tokio::select! {
            // Wait for natural completion
            exit_status = terminal.wait_for_exit() => {
                exit_status
            }
            
            // Handle timeout
            _ = tokio::time::sleep(self.timeout_duration) => {
                // Kill the process
                terminal.kill().await?;
                
                // Get final output
                let output = terminal.get_output().await?;
                
                // Return timeout exit status
                Ok(TerminalExitStatus {
                    exit_code: Some(-1), // Timeout indicator
                    signal: Some("TIMEOUT".to_string()),
                })
            }
        }
    }
}
```

## Testing Requirements
- [ ] Test `terminal/wait_for_exit` blocking behavior until completion
- [ ] Test `terminal/kill` terminates processes correctly
- [ ] Test exit status reporting for normal and signal termination
- [ ] Test timeout pattern implementation with kill and cleanup
- [ ] Test concurrent wait operations on same terminal
- [ ] Test kill operations on already completed processes
- [ ] Test signal handling and platform differences
- [ ] Test graceful vs forceful process termination

## Integration Points
- [ ] Connect to terminal registry and lifecycle management
- [ ] Integrate with process management and signal handling
- [ ] Connect to session validation and access control
- [ ] Integrate with timeout and resource management systems

## Acceptance Criteria
- Complete `terminal/wait_for_exit` implementation with blocking wait
- Complete `terminal/kill` implementation with process termination
- Exit status reporting with exit codes and signals
- Timeout pattern support for command execution limits
- Platform-specific signal handling (Unix/Windows)
- Integration with terminal registry and lifecycle management
- Proper error handling for all process control scenarios
- Comprehensive test coverage for wait and kill operations