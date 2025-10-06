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