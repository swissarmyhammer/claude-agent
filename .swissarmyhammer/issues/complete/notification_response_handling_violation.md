# ACP Spec Variance: Notifications Incorrectly Send Responses

## Issue

The server sends JSON-RPC responses for notifications, which violates both the JSON-RPC 2.0 spec and ACP requirements. Notifications should not receive any response.

## Location

`lib/src/server.rs:327-349`

## Current Implementation

```rust
// Send response
let response = match response_result {
    Ok(result) => {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,  // ❌ Will be null for notifications
            "result": result
        })
    }
    Err(e) => {
        error!("Method {} failed: {}", method, e);
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,  // ❌ Will be null for notifications
            "error": {
                "code": -32603,
                "message": e.to_string()
            }
        })
    }
};

Self::send_response(writer, response).await  // ❌ Always sends response
```

## Problem

The code extracts `id` at line 248:
```rust
let id = request.get("id").cloned();
```

Then ALWAYS sends a response (line 349), even when `id` is `None`, which indicates a notification.

## Expected Behavior Per Specs

### JSON-RPC 2.0 Spec

> A Notification is a Request object without an "id" member... The Server MUST NOT reply to a Notification.

### ACP Spec - session/cancel

From [Protocol Overview](https://agentclientprotocol.com/protocol/overview#notifications):

> **session/cancel** - Cancel ongoing operations (no response expected).

From [Cancellation](https://agentclientprotocol.com/protocol/prompt-turn#cancellation):

> Clients MAY cancel an ongoing prompt turn at any time by sending a `session/cancel` notification.

## Missing Notification Routing

Additionally, `session/cancel` is not explicitly routed in the match statement (lines 257-325). It falls through to the extension method handler, but it's a baseline notification that should be explicitly handled:

```rust
// Missing from match statement:
"session/cancel" => {
    let notification = serde_json::from_value(params)?;
    agent.cancel(notification).await?;
    // ✅ No response for notifications
    return Ok(());
}
```

## Impact

- **Protocol Violation**: Violates JSON-RPC 2.0 specification
- **Spec Non-Compliance**: Violates ACP specification
- **Client Confusion**: Clients may hang waiting for responses that shouldn't exist, or get confused by unexpected `"id": null` responses
- **Interoperability**: May cause issues with strict JSON-RPC 2.0 parsers

## Resolution

1. Check if `id` is `None` (notification) vs `Some` (request)
2. For notifications, execute the method but do NOT send any response
3. Only send responses for requests (when `id.is_some()`)
4. Add explicit routing for `session/cancel` notification

### Proposed Fix

```rust
async fn handle_single_request<W>(
    line: String,
    writer: Arc<tokio::sync::Mutex<W>>,
    agent: Arc<ClaudeAgent>,
) -> crate::Result<()>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    let request: serde_json::Value = serde_json::from_str(&line)?;
    let method = request.get("method")...;
    let id = request.get("id").cloned();
    let params = request.get("params")...;
    
    let is_notification = id.is_none();
    
    // Route to appropriate method
    let response_result = match method {
        // ... existing request handlers ...
        
        // Notification handlers (no response)
        "session/cancel" => {
            let notification = serde_json::from_value(params)?;
            agent.cancel(notification).await?;
            if is_notification {
                return Ok(()); // ✅ No response for notifications
            }
            // Handle as request if id is present (shouldn't happen per spec)
            Ok(serde_json::Value::Null)
        }
        
        // ... other handlers ...
    };
    
    // Only send response for requests (id.is_some())
    if !is_notification {
        let response = match response_result {
            Ok(result) => { /* ... */ }
            Err(e) => { /* ... */ }
        };
        Self::send_response(writer, response).await?;
    }
    
    Ok(())
}
```

## Testing

Add tests for:
1. Notifications should not receive responses
2. `session/cancel` should be handled without sending response
3. Requests (with `id`) should receive responses
4. Extension notifications (methods starting with `_` and no `id`) should not receive responses

## References

- [JSON-RPC 2.0 Specification - Notification](https://www.jsonrpc.org/specification#notification)
- [ACP Protocol Overview - Notifications](https://agentclientprotocol.com/protocol/overview#notifications)
- [ACP Cancellation](https://agentclientprotocol.com/protocol/prompt-turn#cancellation)


## Proposed Solution

After analyzing the server.rs code at lib/src/server.rs:248-349, I've identified the exact problem and solution:

### Root Cause
The `handle_single_request` function extracts the `id` field (line 248) but then ALWAYS sends a response (line 349), regardless of whether `id` is `None` (notification) or `Some(value)` (request). This violates both JSON-RPC 2.0 and ACP specifications.

### Implementation Strategy

1. **Add notification detection**: After extracting `id`, check if it's `None` to determine if this is a notification
2. **Skip response for notifications**: Only call `send_response` when `id.is_some()`
3. **Add explicit session/cancel handler**: Currently not in the match statement, needs to be added as a notification handler
4. **Maintain backward compatibility**: All existing request handling remains unchanged

### Code Changes Required

In `handle_single_request` (lines 232-352):
- Add `let is_notification = id.is_none();` after line 248
- Add `"session/cancel"` case in the match statement (around line 257)
- Wrap the response sending (lines 327-351) in `if !is_notification { ... }`
- Return early from notification handlers without sending responses

### Test Strategy (TDD)

1. Test that notifications (id=None) execute methods but don't send responses
2. Test that session/cancel is handled properly
3. Test that requests (id=Some) continue to receive responses
4. Test that extension notifications work correctly

### Files to Modify
- `lib/src/server.rs` - Main implementation changes
- Add tests to the existing test module at bottom of server.rs

### Notes
- The agent trait methods will still be called for notifications (to perform their actions)
- Only the response sending step is skipped for notifications
- This aligns with the JSON-RPC 2.0 spec requirement: "The Server MUST NOT reply to a Notification"



## Implementation Notes

### Changes Made

1. **Added notification detection** (lib/src/server.rs:280):
   - Added `let is_notification = id.is_none();` after extracting the id field
   - This clearly identifies whether we're handling a notification or a request

2. **Added explicit session/cancel handler** (lib/src/server.rs:326-331):
   - Added explicit match case for "session/cancel" method
   - Calls `agent.cancel(req).await` and returns null result
   - Now properly routed instead of falling through to extension method handler

3. **Skip response for notifications** (lib/src/server.rs:362-375):
   - Added check: `if is_notification { return Ok(()); }`
   - Notifications are executed but no response is sent back
   - Logs success/failure for debugging but doesn't construct JSON-RPC response
   - Only requests (with id field) proceed to response construction and sending

### Test Coverage

Added two comprehensive tests:
- `test_notifications_do_not_receive_responses`: Verifies that notifications don't generate responses
- `test_requests_receive_responses`: Ensures normal request/response flow still works

### Verification

All 685 tests pass, including:
- Existing integration tests
- New notification handling tests
- All server communication tests

### Compliance

The implementation now fully complies with:
- **JSON-RPC 2.0 Specification**: "The Server MUST NOT reply to a Notification"
- **ACP Specification**: session/cancel is properly handled as a notification without response
