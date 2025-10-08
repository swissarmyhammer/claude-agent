# Implement MaxTurnRequests Stop Reason

## Problem

We don't implement turn request limiting as required by the ACP protocol. An agent could make unlimited requests to the language model in a single turn, potentially causing infinite loops with tool calls.

## ACP Specification

From [Prompt Turn spec](https://agentclientprotocol.com/protocol/prompt-turn#stop-reasons):

> **max_turn_requests**: The maximum number of model requests in a single turn is exceeded

The Agent MUST:
1. Track the number of LM requests made in a single prompt turn
2. Stop and return `StopReason::MaxTurnRequests` when limit is exceeded
3. Prevent infinite loops from tool call cycles

## Current Implementation

**Missing**: No turn request counter exists

**Flow** (lib/src/agent.rs):
1. User sends prompt → LM request #1
2. LM requests tool call → execute tool
3. Send tool result back → LM request #2
4. LM requests another tool → execute tool
5. **Loop continues indefinitely** ❌

## Proposed Implementation

### 1. Add Turn Request Counter

In `handle_prompt()` around line 1220 (lib/src/agent.rs):

```rust
async fn handle_prompt(&self, request: PromptRequest) -> Result<PromptResponse, agent_client_protocol::Error> {
    let session_id = &request.session_id;
    
    // ACP compliance: Track turn requests to prevent infinite loops
    let mut turn_request_count = 0;
    const MAX_TURN_REQUESTS: usize = 10; // Configurable limit
    
    // ... existing code ...
}
```

### 2. Increment Counter for Each LM Request

Currently we only make one LM request per turn. But when we implement tool calls properly, we'll need to track:

```rust
// Initial LM request with user prompt
turn_request_count += 1;
if turn_request_count > MAX_TURN_REQUESTS {
    return Ok(PromptResponse {
        stop_reason: StopReason::MaxTurnRequests,
        meta: Some(serde_json::json!({
            "turn_requests": turn_request_count,
            "max_allowed": MAX_TURN_REQUESTS,
        })),
    });
}

let mut stream = self.claude_client
    .query_stream_with_context(&prompt_text, &context)
    .await?;
```

### 3. Check Before Each Subsequent Request

When tool results are sent back for more processing:

```rust
// After tool execution, before sending results back to LM
turn_request_count += 1;
if turn_request_count > MAX_TURN_REQUESTS {
    tracing::warn!(
        "Session {} exceeded max turn requests ({}/{})",
        session_id, turn_request_count, MAX_TURN_REQUESTS
    );
    
    return Ok(PromptResponse {
        stop_reason: StopReason::MaxTurnRequests,
        meta: Some(serde_json::json!({
            "turn_requests": turn_request_count,
            "max_allowed": MAX_TURN_REQUESTS,
            "last_tool_calls": /* list of tools called */,
        })),
    });
}
```

### 4. Make Limit Configurable

Add to AgentConfig (lib/src/config.rs):

```rust
#[derive(Debug, Clone)]
pub struct AgentConfig {
    // ... existing fields ...
    
    /// Maximum number of LM requests allowed in a single turn
    /// Prevents infinite loops from tool call cycles
    /// Default: 10
    pub max_turn_requests: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            max_turn_requests: 10,
        }
    }
}
```

## Tool Call Cycle Example

**Without limit** (current behavior):
```
User: "Debug this infinite loop"
Turn 1: LM → Bash(run code) → crashes
Turn 2: LM → Bash(run code) → crashes  
Turn 3: LM → Bash(run code) → crashes
... infinite loop ❌
```

**With limit** (proposed):
```
User: "Debug this infinite loop"
Turn 1: LM → Bash(run code) → crashes
Turn 2: LM → Bash(run code) → crashes
...
Turn 10: LM → Bash(run code) → crashes
Turn 11: STOP with MaxTurnRequests ✅
Response: "I've attempted 10 tool calls but couldn't resolve the issue"
```

## Integration with Tool Execution

When we properly implement tool call loops (currently tools are executed but we don't loop back to LM), we'll need:

```rust
loop {
    turn_request_count += 1;
    
    if turn_request_count > self.config.max_turn_requests {
        return Ok(PromptResponse {
            stop_reason: StopReason::MaxTurnRequests,
            meta: Some(serde_json::json!({
                "turn_requests": turn_request_count,
            })),
        });
    }
    
    // Send to LM (either initial prompt or tool results)
    let lm_response = /* ... */;
    
    // If LM requests tools, execute them
    if lm_response.has_tool_calls() {
        let tool_results = execute_tools(lm_response.tool_calls);
        // Loop back to send results to LM
        continue;
    }
    
    // LM finished without tools - end turn
    break;
}
```

## Testing

### Unit Test

```rust
#[tokio::test]
async fn test_max_turn_requests_limit() {
    let mut config = AgentConfig::default();
    config.max_turn_requests = 3; // Low limit for testing
    
    let agent = create_test_agent_with_config(config).await;
    
    // Mock LM to always request tool calls (infinite loop scenario)
    // ...
    
    let response = agent.handle_prompt(request).await.unwrap();
    
    assert_eq!(response.stop_reason, StopReason::MaxTurnRequests);
    assert_eq!(response.meta["turn_requests"], 3);
}
```

### Integration Test

Create a prompt that would cause many tool calls:

```rust
#[tokio::test]
async fn test_turn_request_limit_integration() {
    // Prompt that causes multiple tool invocations
    let prompt = "Read file1.txt, then file2.txt, then file3.txt, ... file20.txt";
    
    // With max_turn_requests = 10, should stop after 10 requests
    let response = agent.handle_prompt(prompt).await.unwrap();
    
    assert_eq!(response.stop_reason, StopReason::MaxTurnRequests);
}
```

## Logging

Add tracing for debugging:

```rust
tracing::debug!(
    "Turn request {}/{} for session {}",
    turn_request_count,
    MAX_TURN_REQUESTS,
    session_id
);

if turn_request_count == MAX_TURN_REQUESTS - 2 {
    tracing::warn!(
        "Approaching turn request limit for session {} ({}/{})",
        session_id, turn_request_count, MAX_TURN_REQUESTS
    );
}
```

## Acceptance Criteria

- [ ] Add `max_turn_requests` to AgentConfig
- [ ] Track turn request count in handle_prompt()
- [ ] Check limit before each LM request
- [ ] Return `StopReason::MaxTurnRequests` when exceeded
- [ ] Include metadata about turn counts
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Default limit set to reasonable value (10)
- [ ] Logging added for debugging

## Configuration

Recommended values:
- **Default**: 10 (prevents most infinite loops)
- **Conservative**: 5 (stricter limit)
- **Permissive**: 20 (for complex multi-step tasks)

## References

- ACP Spec: https://agentclientprotocol.com/protocol/prompt-turn#stop-reasons
- Current implementation: lib/src/agent.rs:1200-1430
- Config: lib/src/config.rs

## Related Issues

- See: acp-implement-max-tokens-stop-reason (MaxTokens handling)
- Blocked by: Need proper tool call loop implementation (currently single-shot)
