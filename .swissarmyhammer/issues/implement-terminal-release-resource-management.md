# Implement Terminal Release and Resource Management

## Problem
Our terminal implementation may not properly implement the `terminal/release` method and comprehensive resource cleanup as required by the ACP specification. We need proper terminal disposal, resource cleanup, and lifecycle management.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/terminals:

**terminal/release Method:**
```json
{
  "jsonrpc": "2.0",
  "id": 9,
  "method": "terminal/release",
  "params": {
    "sessionId": "sess_abc123def456",
    "terminalId": "term_xyz789"
  }
}
```

**Release Behavior:**
- Kills the command if still running
- Releases all resources associated with the terminal
- Terminal ID becomes invalid for all other `terminal/*` methods
- If terminal was embedded in tool call, client SHOULD continue displaying output

## Current Issues
- `terminal/release` implementation unclear
- Terminal resource cleanup and disposal unclear
- Terminal ID invalidation after release unclear
- Resource leak prevention for unreleased terminals unclear

## Implementation Tasks

### Terminal Release Method
- [ ] Implement `terminal/release` method handler
- [ ] Add proper JSON-RPC method registration
- [ ] Support terminal disposal and resource cleanup
- [ ] Return null result on successful release

### Resource Cleanup Implementation
- [ ] Kill running processes when terminal is released
- [ ] Clean up output buffers and memory
- [ ] Close file handles and streams
- [ ] Remove terminal from active registry

### Terminal ID Invalidation
- [ ] Mark terminal IDs as invalid after release
- [ ] Reject subsequent operations on released terminals
- [ ] Add proper error handling for invalid terminal IDs
- [ ] Clean up terminal ID references and tracking

### Process Termination on Release
- [ ] Forcefully terminate running processes during release
- [ ] Handle process cleanup and zombie prevention
- [ ] Add graceful vs forceful termination options
- [ ] Support process group termination for child processes

## Terminal Release Implementation
```rust
#[derive(Debug, Deserialize)]
pub struct TerminalReleaseParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

pub async fn handle_terminal_release(
    params: TerminalReleaseParams
) -> Result<serde_json::Value, TerminalError> {
    // Validate session ID
    validate_session_id(&params.session_id)?;
    
    // Get and remove terminal from registry
    let terminal = TERMINAL_REGISTRY.remove(&params.terminal_id)
        .ok_or(TerminalError::TerminalNotFound(params.terminal_id.clone()))?;
    
    // Clean up terminal resources
    terminal.release().await?;
    
    // Return null result per ACP specification
    Ok(serde_json::Value::Null)
}

impl Terminal {
    pub async fn release(mut self) -> Result<(), TerminalError> {
        // Kill process if still running
        if !self.is_finished() {
            self.kill().await?;
        }
        
        // Clean up output buffers
        self.output_buffer.clear();
        
        // Close streams and handles
        self.cleanup_streams().await?;
        
        // Mark as released
        self.state = TerminalState::Released;
        
        Ok(())
    }
    
    async fn cleanup_streams(&mut self) -> Result<(), TerminalError> {
        // Close stdin, stdout, stderr handles
        if let Some(stdin) = self.process.stdin.take() {
            drop(stdin);
        }
        
        // Wait for output capture tasks to complete
        self.join_output_tasks().await?;
        
        Ok(())
    }
}
```

## Implementation Notes
Add terminal release comments:
```rust
// ACP terminal/release method implementation:
// 1. Kill running process if still active
// 2. Clean up all terminal resources (buffers, handles, streams)
// 3. Remove terminal from registry and invalidate ID
// 4. Prevent resource leaks from unreleased terminals
// 5. Return null result on successful release
//
// Proper release prevents resource leaks and ensures clean shutdown.
```

### Terminal State Management
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalState {
    Created,
    Running,
    Finished,
    Released,
}

impl Terminal {
    pub fn is_finished(&self) -> bool {
        matches!(self.state, TerminalState::Finished)
    }
    
    pub fn is_released(&self) -> bool {
        matches!(self.state, TerminalState::Released)
    }
    
    pub fn validate_not_released(&self) -> Result<(), TerminalError> {
        if self.is_released() {
            return Err(TerminalError::TerminalReleased(self.id.clone()));
        }
        Ok(())
    }
}
```

### Terminal Registry Management
```rust
pub struct TerminalRegistry {
    terminals: Arc<Mutex<HashMap<String, Terminal>>>,
    cleanup_interval: Duration,
}

impl TerminalRegistry {
    pub async fn register(&self, terminal_id: String, terminal: Terminal) {
        let mut terminals = self.terminals.lock().await;
        terminals.insert(terminal_id, terminal);
    }
    
    pub async fn remove(&self, terminal_id: &str) -> Option<Terminal> {
        let mut terminals = self.terminals.lock().await;
        terminals.remove(terminal_id)
    }
    
    pub async fn cleanup_orphaned_terminals(&self) {
        // Clean up terminals that haven't been properly released
        let mut terminals = self.terminals.lock().await;
        let mut to_remove = Vec::new();
        
        for (id, terminal) in terminals.iter() {
            if terminal.is_finished() && terminal.should_auto_cleanup() {
                to_remove.push(id.clone());
            }
        }
        
        for id in to_remove {
            if let Some(terminal) = terminals.remove(&id) {
                let _ = terminal.release().await;
            }
        }
    }
}
```

### Resource Leak Prevention
- [ ] Implement automatic cleanup for orphaned terminals
- [ ] Add terminal lifecycle monitoring and alerting
- [ ] Support terminal resource usage tracking
- [ ] Add periodic cleanup of finished terminals

### Error Handling
- [ ] Handle attempts to operate on released terminals
- [ ] Handle process termination failures during release
- [ ] Handle resource cleanup failures gracefully
- [ ] Add proper error responses for invalid operations

## Testing Requirements
- [ ] Test `terminal/release` cleans up all resources
- [ ] Test terminal ID invalidation after release
- [ ] Test process termination during release
- [ ] Test resource leak prevention with unreleased terminals
- [ ] Test concurrent release operations
- [ ] Test error handling for released terminal access
- [ ] Test integration with terminal registry management

## Integration Points
- [ ] Connect to terminal registry and lifecycle management
- [ ] Integrate with process management and cleanup
- [ ] Connect to resource monitoring and leak prevention
- [ ] Integrate with session cleanup and management

## Acceptance Criteria
- Complete `terminal/release` method implementation
- Proper resource cleanup including process termination
- Terminal ID invalidation preventing subsequent operations
- Integration with terminal registry for lifecycle management
- Resource leak prevention with automatic cleanup
- Proper error handling for invalid terminal operations
- Comprehensive test coverage for release scenarios
- Performance optimization for resource cleanup operations