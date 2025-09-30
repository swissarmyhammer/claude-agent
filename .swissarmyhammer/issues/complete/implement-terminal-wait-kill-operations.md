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

## Proposed Solution

After examining the existing terminal_manager.rs code, I can see that the `terminal/wait_for_exit` and `terminal/kill` methods have already been implemented:

1. **wait_for_exit** (lines 573-599): Implements blocking wait for process completion
   - Validates session and terminal
   - Returns cached exit status if already finished
   - Blocks waiting for process completion
   - Updates state to Finished

2. **kill_terminal** (lines 601-623): Implements process termination
   - Validates session and terminal
   - Calls session.kill_process() which handles graceful/forceful termination
   - Logs the kill operation

3. **kill_process** (lines 898-1018): Implements platform-specific process killing
   - Unix: SIGTERM with graceful shutdown timeout, then SIGKILL
   - Windows: Direct TerminateProcess
   - Updates state to Killed
   - Stores exit status with signal information

4. **execute_with_timeout** (lines 625-683): Implements ACP timeout pattern
   - Uses tokio::select! for concurrent wait
   - Automatically kills on timeout
   - Returns TerminalTimeoutResult enum

The implementation is complete and follows ACP specification. My task is to verify all functionality works correctly through comprehensive testing.

## Implementation Approach

1. Review existing tests (lines 1055-1542) to identify gaps
2. Add tests for missing scenarios:
   - Direct wait_for_exit testing with real process
   - Concurrent wait_for_exit calls on same terminal
   - Kill operations on running vs finished processes
   - Platform-specific signal handling verification
   - Edge cases for timeout patterns
3. Run all tests to verify correctness
4. Document any findings in this issue

## Implementation Verification Results

After thorough code review and comprehensive testing, I have verified that the terminal wait and kill operations are fully implemented and working correctly.

### Implementation Status: ✅ COMPLETE

The following ACP methods are fully implemented in `lib/src/terminal_manager.rs`:

1. **terminal/wait_for_exit** (lines 573-599)
   - ✅ Validates session and terminal
   - ✅ Returns cached exit status if already finished
   - ✅ Blocks waiting for process completion
   - ✅ Updates state to Finished
   - ✅ Extracts signal information on Unix systems

2. **terminal/kill** (lines 601-623)
   - ✅ Validates session and terminal
   - ✅ Calls session.kill_process() for process termination
   - ✅ Logs kill operation

3. **kill_process** (lines 898-1018)
   - ✅ Platform-specific implementation (Unix/Windows)
   - ✅ Unix: SIGTERM with graceful shutdown timeout (default 5s)
   - ✅ Unix: Escalates to SIGKILL if graceful shutdown fails
   - ✅ Windows: Direct TerminateProcess
   - ✅ Updates state to Killed
   - ✅ Stores exit status with signal information

4. **execute_with_timeout** (lines 625-683)
   - ✅ Implements ACP timeout pattern
   - ✅ Uses tokio::select! for concurrent wait
   - ✅ Automatically kills process on timeout
   - ✅ Returns TerminalTimeoutResult enum

### Test Coverage Added

Added 4 new comprehensive tests to verify all edge cases:

1. **test_wait_for_exit_with_running_process** (line 1548)
   - Tests direct wait_for_exit with a real running process
   - Verifies exit code and state transition to Finished

2. **test_concurrent_wait_for_exit_calls** (line 1602)
   - Tests multiple concurrent wait_for_exit calls on same terminal
   - Verifies both calls succeed and return same exit status
   - Uses Arc wrapper for thread-safe sharing

3. **test_wait_for_exit_on_released_terminal** (line 1676)
   - Tests wait_for_exit on a released terminal
   - Verifies proper error handling with "Terminal not found"

4. **test_kill_then_wait_for_exit** (line 1710)
   - Tests killing a process then waiting for exit
   - Verifies cached exit status is returned
   - Verifies state is Killed

### Test Results

All 17 terminal_manager tests pass successfully:
```
cargo nextest run --package claude-agent-lib --lib terminal_manager
Summary [1.028s] 17 tests run: 17 passed, 417 skipped
```

### Existing Test Coverage (Verified)

The following scenarios were already well-tested:
- ✅ Terminal state lifecycle
- ✅ Release terminal success/failure
- ✅ Buffer clearing on release
- ✅ Validate not released
- ✅ Get output on released terminal
- ✅ State transitions
- ✅ Timeout triggers kill
- ✅ Concurrent wait and timeout
- ✅ Signal handling graceful termination (Unix)
- ✅ Wait for exit already finished
- ✅ Kill already finished process

### Code Quality

- ✅ All code formatted with `cargo fmt`
- ✅ Platform-specific implementations using cfg attributes
- ✅ Comprehensive error handling
- ✅ Proper state management
- ✅ Clear documentation and comments
- ✅ Thread-safe with Arc and RwLock

### Conclusion

The terminal wait and kill operations are fully implemented according to the ACP specification. All functionality is thoroughly tested and working correctly. No code changes were required - only additional test coverage to verify edge cases.

## Code Review Fixes Applied

### Fix 1: Clippy Warning - Derivable Default Implementation
**File:** `lib/src/terminal_manager.rs:71`

**Issue:** Manual Default implementation for TimeoutConfig was derivable.

**Fix Applied:** Added `Default` to the derive attributes:
```rust
#[derive(Debug, Clone, Default)]
pub struct TimeoutConfig {
    pub default_execution_timeout: Option<Duration>,
    pub graceful_shutdown_timeout: GracefulShutdownTimeout,
    pub command_timeouts: HashMap<String, Duration>,
}
```

**Result:** ✅ Clippy warning eliminated, all clippy checks pass with `-D warnings`

### Fix 2: Memory Leak in test_terminal_state_lifecycle
**File:** `lib/src/terminal_manager.rs:1082`

**Issue:** Test created a terminal process but never cleaned it up, causing nextest to report a leak.

**Root Cause:** The test verified terminal state but didn't release the terminal, leaving the `echo` process and its resources uncleaned.

**Fix Applied:** Added proper cleanup by releasing the terminal at test end:
```rust
// Clean up: release the terminal to avoid resource leak
let params = TerminalReleaseParams {
    session_id,
    terminal_id,
};
let _ = manager.release_terminal(&session_manager, params).await;
```

**Result:** ✅ Memory leak eliminated, all 17 terminal_manager tests pass without leaks

### Verification

All fixes verified with:
- `cargo nextest run` - All 457 tests pass, no leaks in terminal_manager tests
- `cargo clippy --all-targets -- -D warnings` - No warnings
- `cargo fmt` - Code properly formatted

### Summary

Both code review issues have been successfully resolved:
1. ✅ Clippy derivable_impls warning fixed
2. ✅ Memory leak in test_terminal_state_lifecycle fixed

The implementation is now clean with no warnings, no leaks, and all tests passing.