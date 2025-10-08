# Verify Tool Call Status Updates

## Problem

Need to verify that we send all required tool call status updates as specified by the ACP protocol. The spec requires three distinct status updates per tool call, but we need to confirm our implementation sends them all.

## ACP Specification

From [Prompt Turn spec](https://agentclientprotocol.com/protocol/prompt-turn#5-tool-invocation-and-status-reporting):

The Agent MUST send status updates for each tool call:

### 1. Initial Tool Call (pending)
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123",
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "call_001",
      "title": "Analyzing Python code",
      "kind": "other",
      "status": "pending"
    }
  }
}
```

### 2. Execution Start (in_progress)
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "call_001",
      "status": "in_progress"
    }
  }
}
```

### 3. Completion (completed/failed)
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "call_001",
      "status": "completed",
      "content": [
        {
          "type": "content",
          "content": {
            "type": "text",
            "text": "Analysis complete..."
          }
        }
      ]
    }
  }
}
```

## Investigation Required

### Check Tool Call Handler (lib/src/tools.rs)

Find where tool calls are executed and verify we send all three updates:

```rust
// Expected pattern:
async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult> {
    // 1. Send initial pending status
    self.send_notification(SessionUpdate::ToolCall {
        tool_call_id: tool_call.id.clone(),
        title: format!("Executing {}", tool_call.name),
        kind: ToolCallKind::Other,
        status: ToolCallStatus::Pending,
    }).await?;
    
    // 2. Send in_progress status
    self.send_notification(SessionUpdate::ToolCallUpdate {
        tool_call_id: tool_call.id.clone(),
        status: ToolCallStatus::InProgress,
        content: None,
    }).await?;
    
    // 3. Execute tool
    let result = /* ... */;
    
    // 4. Send completed status with result
    self.send_notification(SessionUpdate::ToolCallUpdate {
        tool_call_id: tool_call.id.clone(),
        status: ToolCallStatus::Completed,
        content: Some(vec![/* result content */]),
    }).await?;
    
    Ok(result)
}
```

### Check Agent Integration (lib/src/agent.rs)

Verify tool call notifications are sent from agent:

```bash
grep -n "ToolCall\|ToolCallUpdate" lib/src/agent.rs
```

Look for:
- `SessionUpdate::ToolCall` - initial notification
- `SessionUpdate::ToolCallUpdate` - status updates
- Proper status progression: pending → in_progress → completed/failed

### Check Protocol Types

Verify we have all required types defined:

```rust
// In agent_client_protocol or our types
pub enum SessionUpdate {
    ToolCall {
        tool_call_id: String,
        title: String,
        kind: ToolCallKind,
        status: ToolCallStatus,
    },
    ToolCallUpdate {
        tool_call_id: String,
        status: ToolCallStatus,
        content: Option<Vec<ContentBlock>>,
    },
    // ...
}

pub enum ToolCallStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}
```

## Testing Strategy

### 1. Manual Test with Real Claude

Run our agent and watch for tool call notifications:

```bash
# In one terminal - start agent with debug logging
RUST_LOG=debug cargo run --bin claude-agent-cli

# In another terminal - send request that triggers tool call
# Watch logs for:
# - "Sending ToolCall notification with status: pending"
# - "Sending ToolCallUpdate notification with status: in_progress"  
# - "Sending ToolCallUpdate notification with status: completed"
```

### 2. Unit Test for Status Progression

Add test in lib/src/tools.rs or lib/src/agent.rs:

```rust
#[tokio::test]
async fn test_tool_call_sends_all_status_updates() {
    let (agent, mut notification_receiver) = create_test_agent_with_notifications().await;
    
    // Trigger tool call
    let request = create_prompt_with_tool_call();
    let response_future = agent.handle_prompt(request);
    
    // Collect notifications
    let mut notifications = Vec::new();
    loop {
        tokio::select! {
            Some(notif) = notification_receiver.recv() => {
                notifications.push(notif);
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                break;
            }
        }
    }
    
    // Verify sequence
    assert_eq!(notifications.len(), 3);
    
    // 1. Initial pending
    assert!(matches!(
        notifications[0].update,
        SessionUpdate::ToolCall { status: ToolCallStatus::Pending, .. }
    ));
    
    // 2. In progress
    assert!(matches!(
        notifications[1].update,
        SessionUpdate::ToolCallUpdate { status: ToolCallStatus::InProgress, .. }
    ));
    
    // 3. Completed with content
    assert!(matches!(
        notifications[2].update,
        SessionUpdate::ToolCallUpdate { 
            status: ToolCallStatus::Completed,
            content: Some(_),
            ..
        }
    ));
}
```

### 3. Integration Test

Test with real Claude CLI process:

```rust
#[tokio::test]
async fn test_tool_call_status_integration() {
    let agent = create_real_agent().await;
    
    // Send prompt that will trigger tool use
    let prompt = "List files in the current directory";
    
    // Track notifications
    let notifications = collect_notifications_during_prompt(agent, prompt).await;
    
    // Verify we got all three status updates for each tool call
    let tool_call_notifications: Vec<_> = notifications.iter()
        .filter(|n| matches!(n.update, SessionUpdate::ToolCall { .. } | SessionUpdate::ToolCallUpdate { .. }))
        .collect();
    
    // Should have: 1 ToolCall + 2 ToolCallUpdate per tool
    assert!(tool_call_notifications.len() >= 3);
}
```

## Possible Issues to Fix

### If Missing Initial `ToolCall`:
- Add `SessionUpdate::ToolCall` when tool is first requested
- Include tool name in title
- Set status to `Pending`

### If Missing `InProgress` Update:
- Add `SessionUpdate::ToolCallUpdate` with status `InProgress` before execution
- Send immediately after starting tool execution

### If Missing `Completed` Update:
- Add `SessionUpdate::ToolCallUpdate` with status `Completed` after execution
- Include tool result in content field

### If Sending Wrong Update Types:
- Use `ToolCall` for initial notification (has `title` and `kind`)
- Use `ToolCallUpdate` for status changes (no `title` or `kind`)

## Acceptance Criteria

- [ ] Verified we send initial `ToolCall` notification with status `Pending`
- [ ] Verified we send `ToolCallUpdate` with status `InProgress` 
- [ ] Verified we send `ToolCallUpdate` with status `Completed` and content
- [ ] Verified proper tool call ID tracking
- [ ] All three notifications sent for each tool call
- [ ] Unit test covers status progression
- [ ] Integration test with real Claude passes
- [ ] Document findings in memo

## Documentation

After verification, update memo with findings:
- What we found (compliant or not)
- What was fixed (if anything)
- Code locations for tool call handling
- Test coverage added

## References

- ACP Spec: https://agentclientprotocol.com/protocol/prompt-turn#5-tool-invocation-and-status-reporting
- Tool handler: lib/src/tools.rs
- Agent: lib/src/agent.rs
- Protocol types: Check agent_client_protocol crate

## Related Issues

- Part of overall ACP compliance verification
- May block: proper tool call loop implementation
