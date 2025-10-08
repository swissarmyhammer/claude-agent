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



## Analysis of Current Implementation

After examining the codebase, I found that **MaxTurnRequests is already partially implemented**:

### What's Already Done ✅

1. **Configuration** (lib/src/config.rs:26-29, 50-52):
   - `max_turn_requests` field exists with default value of 50
   - Properly serializable and configurable

2. **Session Tracking** (lib/src/session.rs:213, 295-317):
   - `turn_request_count: u64` field tracks requests
   - `increment_turn_requests()` increments and returns count
   - `reset_turn_counters()` resets counter
   - `get_turn_request_count()` retrieves current count

3. **Non-Streaming Enforcement** (lib/src/agent.rs:2833-2848):
   - Checks turn request limit BEFORE making Claude API call
   - Returns `StopReason::MaxTurnRequests` when exceeded
   - Includes metadata with turn_requests, max_turn_requests, session_id

4. **Unit Tests** (lib/src/agent.rs:6410-6530):
   - Tests for turn request counting
   - Tests for turn counter reset

### What's Missing ❌

1. **Turn Counter Reset Logic**:
   - Counter is incremented but never reset in production code
   - According to ACP spec, counters should reset at the start of each new prompt turn
   - Currently only reset in test code

2. **Streaming Flow** (lib/src/agent.rs:1236-1450):
   - The streaming path (`handle_streaming_prompt`) does NOT check turn request limits
   - Only non-streaming path has the check

3. **Documentation**:
   - No clear explanation of when a "turn" begins and ends
   - Missing documentation on the relationship between turn requests and tool call loops

## Root Cause Analysis

The implementation has the infrastructure but is incomplete:
- Turn counters increment but never reset → counter grows unbounded across multiple prompt turns
- Streaming path bypasses the check → limit only enforced for non-streaming

## Proposed Solution

### 1. Reset Turn Counters at Turn Start

Add turn counter reset at the beginning of `handle_prompt()` around line 2800:

```rust
async fn handle_prompt(&self, request: PromptRequest) -> Result<PromptResponse, agent_client_protocol::Error> {
    // ... existing validation code ...
    
    // Reset turn counters at the start of each new turn
    // ACP defines a turn as a single user prompt and all subsequent LM requests until final response
    self.session_manager
        .update_session(&session_id, |session| {
            session.reset_turn_counters();
        })
        .map_err(|_| agent_client_protocol::Error::internal_error())?;
    
    // Add user message to session
    let user_message = crate::session::Message {
        // ...
    };
}
```

### 2. Add Turn Request Check to Streaming Path

In `handle_streaming_prompt()` around line 1290, add the same check that exists in non-streaming:

```rust
async fn handle_streaming_prompt(
    &self,
    session_id: &crate::session::SessionId,
    request: &PromptRequest,
    session: &crate::session::Session,
) -> Result<PromptResponse, agent_client_protocol::Error> {
    // ... existing content validation ...
    
    // Check turn request limit (same as non-streaming path)
    let mut updated_session = session.clone();
    let current_requests = updated_session.increment_turn_requests();
    if current_requests > self.config.max_turn_requests {
        tracing::info!(
            "Turn request limit exceeded ({} > {}) for session: {} (streaming)",
            current_requests,
            self.config.max_turn_requests,
            session_id
        );
        return Ok(PromptResponse {
            stop_reason: StopReason::MaxTurnRequests,
            meta: Some(serde_json::json!({
                "turn_requests": current_requests,
                "max_turn_requests": self.config.max_turn_requests,
                "session_id": session_id.to_string(),
                "streaming": true
            })),
        });
    }
    
    // Update session with incremented counter
    self.session_manager
        .update_session(session_id, |s| {
            s.turn_request_count = updated_session.turn_request_count;
        })
        .map_err(|_| agent_client_protocol::Error::internal_error())?;
    
    // ... continue with streaming ...
}
```

### 3. Add Integration Tests

Test scenarios:
- Turn counter resets between prompt turns
- Turn counter increments within a turn
- Limit enforced in streaming path
- Limit enforced in non-streaming path
- Metadata correctly reports counts

## Implementation Plan

1. Write failing test for turn counter reset between turns
2. Implement turn counter reset at start of handle_prompt()
3. Write failing test for streaming path limit enforcement
4. Implement turn request check in handle_streaming_prompt()
5. Run full test suite
6. Update documentation




## Test Design Issue

After implementing the turn counter reset and streaming path check, I discovered an important insight:

**Current Implementation Reality:**
- Each call to `prompt()` represents a NEW turn (resets counters)
- Within each `prompt()` call, we only make ONE LM request
- We don't currently have tool call loops that would make multiple LM requests per turn

**This means:**
- The MaxTurnRequests limit cannot be exceeded in the current implementation
- The feature is correctly implemented but not yet exercised  
- When tool call loops are implemented in the future, this limit will prevent infinite loops

**Test Strategy:**
Rather than trying to test a scenario that doesn't exist yet (tool call loops), the tests should verify:
1. ✅ Turn counters reset between prompts
2. ✅ Streaming path has the same limit check as non-streaming  
3. ✅ The check happens before making LM requests
4. ✅ Correct metadata is returned when limit exceeded

The second test should be simplified to just verify the streaming path has the check, not try to trigger it in a real scenario.




## Code Review Improvements Completed

### Changes Made

1. **Enhanced streaming path documentation** (lib/src/agent.rs:1295)
   - Added comprehensive comment explaining ACP compliance requirement
   - Documented relationship to non-streaming path check
   - Explained future behavior with tool call loops

2. **Enhanced test documentation** (lib/src/agent.rs:8131-8138)
   - Added TODO comment outlining future test enhancements
   - Explained current limitation (max_turn_requests=0 approach)
   - Documented what should be tested when tool call loops exist

3. **Improved turn counter reset comment** (lib/src/agent.rs:2838-2840)
   - Reformatted as proper paragraph comment for consistency
   - Clarified purpose of preventing unbounded counter growth
   - Better explained ACP turn definition

### Test Results

All tests pass: 713 tests run, 713 passed
- No regressions introduced
- Existing MaxTurnRequests tests continue to pass
- Code properly formatted and documented

### Decision Rationale

**Why these specific improvements:**
- Documentation ensures future maintainers understand the ACP protocol requirements
- TODO comments guide future enhancement when tool call loops are implemented
- Consistent comment formatting improves code readability

**Why low priority items were deferred:**
- Edge case test for u64::MAX - Current tests adequately cover the feature
- Assertion message improvements - Current messages are clear enough
- Session cloning optimization - Consistency with non-streaming path is more valuable

The implementation is now ready with improved documentation while maintaining the clean, working implementation.
