# Implement Terminal Timeout and Process Control

## Problem
Our terminal implementation may not support the command timeout pattern and comprehensive process control as described in the ACP specification. We need proper timeout handling, signal management, and process lifecycle control.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/terminals:

**Timeout Pattern Implementation:**
1. Create terminal with `terminal/create`
2. Start timer for desired timeout duration
3. Concurrently wait for either timer to expire or `terminal/wait_for_exit` to return
4. If timer expires first:
   - Call `terminal/kill` to terminate the command
   - Call `terminal/output` to retrieve any final output
   - Include the output in the response to the model
5. Call `terminal/release` when done

**Process Control Requirements:**
- Signal handling for process termination
- Exit code and signal reporting
- Process group management
- Graceful vs forceful termination

## Current Issues
- Timeout pattern implementation for command execution unclear
- Process signal handling and termination unclear
- Concurrent timeout and exit waiting unclear
- Process control and lifecycle management unclear

## Implementation Tasks

### Timeout Pattern Implementation
- [ ] Implement command timeout utilities using tokio::select!
- [ ] Support concurrent waiting for timeout vs process completion
- [ ] Add automatic kill and cleanup on timeout
- [ ] Include timeout information in exit status

### Process Signal Handling
- [ ] Implement platform-specific signal handling (Unix/Windows)
- [ ] Support graceful termination with SIGTERM
- [ ] Add forceful termination with SIGKILL as fallback
- [ ] Handle signal propagation to process groups

### Process Group Management
- [ ] Create process groups for child processes
- [ ] Support killing entire process trees
- [ ] Handle orphaned child processes
- [ ] Add process group cleanup and monitoring

### Concurrent Operation Support
- [ ] Support concurrent wait and timeout operations
- [ ] Handle race conditions between kill and natural exit
- [ ] Add proper synchronization for process state changes
- [ ] Support multiple concurrent operations per terminal

## Timeout Implementation
```rust
pub struct TerminalTimeout {
    terminal_id: String,
    timeout_duration: Duration,
}

impl TerminalTimeout {
    pub async fn execute_with_timeout(
        &self,
        terminal_manager: &TerminalManager,
    ) -> Result<TerminalTimeoutResult, TerminalError> {
        let terminal = terminal_manager.get_terminal(&self.terminal_id)?;
        
        tokio::select! {
            // Wait for natural completion
            exit_status = terminal.wait_for_exit() => {
                match exit_status {
                    Ok(status) => Ok(TerminalTimeoutResult::Completed(status)),
                    Err(e) => Err(TerminalError::WaitFailed(e)),
                }
            }
            
            // Handle timeout
            _ = tokio::time::sleep(self.timeout_duration) => {
                // Kill the process
                terminal.kill().await?;
                
                // Get final output
                let output_result = terminal.get_output().await?;
                
                Ok(TerminalTimeoutResult::TimedOut {
                    output: output_result.output,
                    truncated: output_result.truncated,
                })
            }
        }
    }
}

#[derive(Debug)]
pub enum TerminalTimeoutResult {
    Completed(TerminalExitStatus),
    TimedOut {
        output: String,
        truncated: bool,
    },
}
```

## Implementation Notes
Add timeout and process control comments:
```rust
// ACP terminal timeout and process control implementation:
// 1. Concurrent timeout and exit waiting using tokio::select!
// 2. Automatic process kill when timeout exceeded
// 3. Final output retrieval for timeout scenarios
// 4. Platform-specific signal handling (SIGTERM/SIGKILL on Unix)
// 5. Process group management for child process cleanup
//
// Timeout pattern prevents hanging operations and provides resource control.
```

### Signal Handling Implementation
```rust
impl ProcessController {
    pub async fn terminate_gracefully(&self, process: &mut Child) -> Result<(), ProcessError> {
        #[cfg(unix)]
        {
            use nix::sys::signal::{self, Signal};
            use nix::unistd::Pid;
            
            let pid = Pid::from_raw(process.id() as i32);
            
            // Send SIGTERM for graceful shutdown
            signal::kill(pid, Signal::SIGTERM)
                .map_err(|e| ProcessError::SignalFailed(e))?;
            
            // Wait for graceful shutdown with timeout
            match tokio::time::timeout(Duration::from_secs(5), process.wait()).await {
                Ok(Ok(_)) => return Ok(()), // Graceful shutdown successful
                Ok(Err(e)) => return Err(ProcessError::WaitFailed(e)),
                Err(_) => {
                    // Timeout - force kill with SIGKILL
                    signal::kill(pid, Signal::SIGKILL)
                        .map_err(|e| ProcessError::SignalFailed(e))?;
                }
            }
        }
        
        #[cfg(windows)]
        {
            // Windows doesn't have signals - use TerminateProcess
            process.kill().await
                .map_err(|e| ProcessError::KillFailed(e))?;
        }
        
        Ok(())
    }
}
```

### Timeout Configuration and Management
- [ ] Add configurable default timeout durations
- [ ] Support per-command timeout configuration
- [ ] Add timeout escalation strategies (SIGTERM → SIGKILL)
- [ ] Implement timeout monitoring and alerting

### Process State Tracking
- [ ] Track process states (starting, running, finished, killed, timed_out)
- [ ] Handle process state transitions and validation
- [ ] Add process state synchronization across concurrent operations
- [ ] Support process state persistence and recovery

### Error Handling and Recovery
- [ ] Handle timeout scenarios with proper cleanup
- [ ] Handle kill operation failures
- [ ] Handle signal delivery failures
- [ ] Add process orphan detection and cleanup

## Testing Requirements
- [ ] Test timeout pattern with natural process completion
- [ ] Test timeout pattern with timeout triggering kill
- [ ] Test concurrent timeout and exit waiting
- [ ] Test signal handling and process termination
- [ ] Test process group management and cleanup
- [ ] Test timeout configuration and customization
- [ ] Test error scenarios and recovery mechanisms

## Integration Points
- [ ] Connect to terminal creation and management
- [ ] Integrate with tool call execution and reporting
- [ ] Connect to process management and lifecycle
- [ ] Integrate with session timeout and resource management

## Configuration Support
- [ ] Add configurable timeout durations per command type
- [ ] Support global and per-session timeout policies
- [ ] Configure signal escalation strategies
- [ ] Add timeout monitoring and alerting configuration

## Acceptance Criteria
- Complete timeout pattern implementation with concurrent wait/timeout
- Platform-specific signal handling for graceful and forceful termination
- Process group management and child process cleanup
- Integration with terminal lifecycle and management
- Configurable timeout durations and escalation strategies
- Proper error handling for timeout and process control scenarios
- Comprehensive test coverage for all timeout and control scenarios
- Performance optimization for concurrent operations