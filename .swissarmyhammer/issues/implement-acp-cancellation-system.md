# Implement ACP Cancellation System

## Problem
Our prompt processing doesn't implement the complete cancellation system required by the ACP specification. We need to handle `session/cancel` notifications, immediately cancel all ongoing operations, and respond with proper `cancelled` stop reasons.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn:

**Cancellation Flow:**
1. Client sends `session/cancel` notification
2. Client marks all pending tool calls as `cancelled`
3. Client responds to pending `session/request_permission` with `cancelled`
4. Agent stops all operations and sends final updates
5. Agent responds to original `session/prompt` with `cancelled` stop reason

**Cancellation Notification:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/cancel",
  "params": {
    "sessionId": "sess_abc123def456"
  }
}
```

**Final Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "stopReason": "cancelled"
  }
}
```

## Current Issues
- Missing `session/cancel` notification handling
- No immediate cancellation of ongoing operations
- Missing proper `cancelled` stop reason response
- No tool call cancellation coordination
- Missing permission request cancellation handling

## Implementation Tasks

### Cancellation Notification Handling
- [ ] Register `session/cancel` notification handler
- [ ] Validate session ID in cancellation requests
- [ ] Trigger immediate cancellation of all session operations
- [ ] Log cancellation requests for debugging and monitoring

### Operation Cancellation Coordination
- [ ] Cancel all ongoing language model requests immediately
- [ ] Cancel all in-progress tool executions
- [ ] Cancel any pending permission requests
- [ ] Stop any streaming operations or file transfers
- [ ] Coordinate cancellation across multiple concurrent operations

### Tool Call Cancellation
- [ ] Mark all pending/in-progress tool calls as `cancelled`
- [ ] Send `tool_call_update` notifications with `cancelled` status
- [ ] Clean up tool execution resources and processes
- [ ] Handle tool call cancellation gracefully without data corruption

### Permission Request Cancellation
- [ ] Respond to pending `session/request_permission` with `cancelled`
- [ ] Clean up permission request state
- [ ] Handle permission cancellation in tool execution flow
- [ ] Coordinate permission cancellation with client expectations

### Language Model Request Cancellation
- [ ] Implement cancellation of ongoing LM API calls
- [ ] Handle partial responses from cancelled LM requests
- [ ] Clean up streaming connections and resources
- [ ] Ensure no orphaned LM requests continue processing

## Cancellation State Management
```rust
#[derive(Debug, Clone)]
pub struct CancellationState {
    pub cancelled: bool,
    pub cancellation_time: SystemTime,
    pub cancelled_operations: HashSet<String>,
    pub pending_cleanup: Vec<CleanupTask>,
}

impl CancellationState {
    pub fn cancel_all(&mut self) -> Vec<CancellationTask>;
    pub fn is_cancelled(&self) -> bool;
    pub fn add_cancelled_operation(&mut self, operation_id: String);
}
```

## Implementation Notes
Add cancellation system comments:
```rust
// ACP requires immediate and comprehensive cancellation handling:
// 1. Process session/cancel notifications immediately
// 2. Cancel ALL ongoing operations (LM, tools, permissions)
// 3. Send final status updates before responding
// 4. Respond to original session/prompt with cancelled stop reason
// 5. Clean up all resources and prevent orphaned operations
//
// Cancellation must be fast and reliable to maintain responsiveness.
```

### Immediate Response Requirements
- [ ] Process cancellation notifications with highest priority
- [ ] Interrupt blocking operations immediately
- [ ] Set cancellation flags before starting cleanup
- [ ] Ensure cancellation state is thread-safe
- [ ] Handle concurrent cancellation requests

### Final Update Coordination
- [ ] Send all pending `session/update` notifications before responding
- [ ] Include final tool call status updates
- [ ] Send any remaining progress updates
- [ ] Ensure proper ordering of final notifications
- [ ] Handle client disconnection during final updates

### Resource Cleanup
- [ ] Clean up file handles and temporary files
- [ ] Terminate child processes spawned by tools
- [ ] Close network connections and streams
- [ ] Release memory and computational resources
- [ ] Clean up session state and caches

### Error Handling During Cancellation
- [ ] Handle errors during cancellation gracefully
- [ ] Ensure cancellation completes even if cleanup fails
- [ ] Log cancellation errors without blocking response
- [ ] Prevent cancellation errors from causing deadlocks
- [ ] Support partial cancellation scenarios

## Client-Side Coordination
The implementation must coordinate with expected client behavior:
- [ ] Handle clients that mark tool calls as cancelled preemptively
- [ ] Support clients that cancel permission requests immediately
- [ ] Coordinate with client UI updates during cancellation
- [ ] Handle network issues during cancellation

## Testing Requirements
- [ ] Test basic session cancellation with immediate response
- [ ] Test cancellation during tool execution with proper status updates
- [ ] Test cancellation during language model requests
- [ ] Test cancellation during permission requests
- [ ] Test concurrent operation cancellation
- [ ] Test resource cleanup after cancellation
- [ ] Test cancellation error scenarios and recovery
- [ ] Test client disconnection during cancellation

## Performance Requirements
- [ ] Cancellation response time under 100ms
- [ ] Efficient cancellation of large numbers of operations
- [ ] Non-blocking cancellation implementation
- [ ] Minimal resource usage during cancellation
- [ ] Fast cleanup of cancelled operations

## Integration Points
- [ ] Connect to language model API cancellation mechanisms
- [ ] Integrate with tool execution cancellation
- [ ] Connect to permission request system
- [ ] Integrate with session update notification system
- [ ] Connect to session management for state cleanup

## Edge Cases and Error Scenarios
- [ ] Handle cancellation of already completed operations
- [ ] Handle double cancellation requests
- [ ] Handle cancellation during session loading
- [ ] Handle cancellation with no active operations
- [ ] Handle cancellation during initialization

## Acceptance Criteria
- `session/cancel` notifications processed immediately
- All ongoing operations cancelled within 100ms
- Final `session/update` notifications sent before response
- Original `session/prompt` responds with `cancelled` stop reason
- Tool calls marked as `cancelled` with proper status updates
- Permission requests cancelled with `cancelled` outcome
- Comprehensive resource cleanup completed
- No orphaned operations or resource leaks
- Comprehensive test coverage for all cancellation scenarios
- Integration with existing session and tool management systems