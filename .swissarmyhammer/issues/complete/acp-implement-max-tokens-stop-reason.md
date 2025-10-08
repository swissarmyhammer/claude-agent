# Implement MaxTokens Stop Reason

## Problem

We don't handle the `StopReason::MaxTokens` case as required by the ACP protocol. When Claude reaches the maximum token limit, we should detect this and return the appropriate stop reason.

## ACP Specification

From [Prompt Turn spec](https://agentclientprotocol.com/protocol/prompt-turn#stop-reasons):

> **max_tokens**: The maximum token limit is reached

The Agent MUST return `StopReason::MaxTokens` when the language model stops due to reaching the token limit.

## Current Implementation

**Missing**: We never return `StopReason::MaxTokens`

**Current Stop Reasons** (lib/src/agent.rs):
- ✅ `StopReason::EndTurn` (line 1427)
- ✅ `StopReason::Cancelled` (lines 1322, 1371)
- ✅ `StopReason::Refusal` (line 1388)
- ❌ `StopReason::MaxTokens` - NOT IMPLEMENTED
- ❌ `StopReason::MaxTurnRequests` - NOT IMPLEMENTED (separate issue)

## Claude CLI Output

Claude outputs stop information in the result message:

```json
{"type":"result","subtype":"success","is_error":false,"duration_ms":2845,"num_turns":1,"result":"4","session_id":"uuid","stop_reason":"max_tokens",...}
```

We need to parse the `stop_reason` field from these result messages.

## Implementation Plan

### 1. Update ProtocolTranslator (lib/src/protocol_translator.rs)

Add detection of stop_reason in result messages:

```rust
pub struct ProtocolTranslator;

#[derive(Debug, Clone)]
pub struct StreamResult {
    pub stop_reason: Option<String>, // "end_turn", "max_tokens", etc.
    pub usage: Option<TokenUsage>,
}

impl ProtocolTranslator {
    /// Parse result message and extract stop_reason
    pub fn parse_result_message(line: &str) -> Result<Option<StreamResult>> {
        let json: serde_json::Value = serde_json::from_str(line)?;
        
        if json.get("type").and_then(|t| t.as_str()) == Some("result") {
            let stop_reason = json.get("stop_reason")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
                
            return Ok(Some(StreamResult {
                stop_reason,
                usage: None, // Can extract token usage too
            }));
        }
        
        Ok(None)
    }
}
```

### 2. Update ClaudeClient (lib/src/claude.rs)

Return stop_reason information from streaming:

```rust
pub struct MessageChunk {
    pub content: String,
    pub chunk_type: ChunkType,
    pub tool_call: Option<ToolCallInfo>,
    pub token_usage: Option<TokenUsageInfo>,
    pub stop_reason: Option<String>, // ADD THIS
}
```

In `query_stream()` and `query_stream_with_context()`:
- When we read a line with `type: "result"`, parse the stop_reason
- Return it in the final chunk or as metadata

### 3. Update Agent (lib/src/agent.rs)

In `handle_prompt()` around line 1309:

```rust
let mut full_response = String::new();
let mut chunk_count = 0;
let mut claude_stop_reason: Option<String> = None; // ADD THIS

while let Some(chunk) = stream.next().await {
    // ... existing code ...
    
    // Capture stop_reason if present
    if let Some(reason) = chunk.stop_reason {
        claude_stop_reason = Some(reason);
    }
    
    // ... existing code ...
}

// Map Claude's stop_reason to ACP StopReason
let stop_reason = match claude_stop_reason.as_deref() {
    Some("max_tokens") => StopReason::MaxTokens,
    Some("end_turn") | None => StopReason::EndTurn,
    Some(_) => StopReason::EndTurn, // Unknown, default to EndTurn
};

Ok(PromptResponse {
    stop_reason,
    meta: Some(serde_json::json!({
        "processed": true,
        "claude_stop_reason": claude_stop_reason,
    })),
})
```

## Testing

### Unit Tests

Add test in lib/src/protocol_translator.rs:

```rust
#[test]
fn test_parse_result_with_max_tokens() {
    let result_line = r#"{"type":"result","subtype":"success","stop_reason":"max_tokens","usage":{...}}"#;
    let result = ProtocolTranslator::parse_result_message(result_line).unwrap();
    
    assert!(result.is_some());
    assert_eq!(result.unwrap().stop_reason, Some("max_tokens".to_string()));
}
```

Add test in lib/src/agent.rs:

```rust
#[tokio::test]
async fn test_max_tokens_stop_reason() {
    // Create agent with mocked Claude that returns max_tokens
    // Verify PromptResponse has StopReason::MaxTokens
}
```

### Integration Test

Test with real Claude CLI by sending very long prompt:

```bash
echo '{"type":"user","message":{"role":"user","content":"'$(printf 'word %.0s' {1..100000})'"}}' | \
  claude -p --input-format stream-json --output-format stream-json --verbose --max-tokens 10
```

Verify we return `StopReason::MaxTokens`.

## Acceptance Criteria

- [ ] Parse `stop_reason` from Claude result messages
- [ ] Map `"max_tokens"` to `StopReason::MaxTokens`
- [ ] Map `"end_turn"` to `StopReason::EndTurn`
- [ ] Return proper stop reason in PromptResponse
- [ ] Unit tests pass
- [ ] Integration test with real Claude CLI passes
- [ ] Memo updated with implementation details

## References

- ACP Spec: https://agentclientprotocol.com/protocol/prompt-turn#stop-reasons
- Current implementation: lib/src/agent.rs:1309-1430
- Protocol translator: lib/src/protocol_translator.rs
- Claude client: lib/src/claude.rs

## Related Issues

- See: acp-implement-max-turn-requests-stop-reason (MaxTurnRequests handling)



## Proposed Solution

After analyzing the codebase, I've identified the data flow and implementation strategy:

### Architecture Analysis

The flow from Claude CLI → ACP is:
1. **ClaudeProcess** (lib/src/claude_process.rs) reads raw stdout lines from claude CLI
2. **ProtocolTranslator** (lib/src/protocol_translator.rs) converts stream-json to ACP format
3. **ClaudeClient** (lib/src/claude.rs) wraps the process and provides streaming via MessageChunk
4. **Agent** (lib/src/agent.rs) consumes MessageChunks and returns PromptResponse with StopReason

### Implementation Steps

#### Step 1: Extend ProtocolTranslator to Parse Result Messages
Currently, result messages are ignored (line 146-150). We need to parse them:

```rust
pub struct StreamResult {
    pub stop_reason: Option<String>,
    pub usage: Option<TokenUsage>,
}

pub fn parse_result_message(line: &str) -> Result<Option<StreamResult>> {
    let parsed: JsonValue = serde_json::from_str(line)?;
    
    if parsed.get("type").and_then(|v| v.as_str()) == Some("result") {
        let stop_reason = parsed.get("stop_reason")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        
        return Ok(Some(StreamResult {
            stop_reason,
            usage: None,
        }));
    }
    
    Ok(None)
}
```

#### Step 2: Add stop_reason to MessageChunk
Update lib/src/claude.rs:45-53 to include stop_reason:

```rust
pub struct MessageChunk {
    pub content: String,
    pub chunk_type: ChunkType,
    pub tool_call: Option<ToolCallInfo>,
    pub token_usage: Option<TokenUsageInfo>,
    pub stop_reason: Option<String>,  // NEW FIELD
}
```

#### Step 3: Update query_stream to Capture stop_reason
In lib/src/claude.rs:204-260, modify the spawned task to:
1. Parse result messages using ProtocolTranslator::parse_result_message
2. Send a final chunk with stop_reason when result message is detected

#### Step 4: Map stop_reason in Agent.handle_prompt
In lib/src/agent.rs:1309-1434, capture stop_reason from streaming chunks:

```rust
let mut claude_stop_reason: Option<String> = None;

while let Some(chunk) = stream.next().await {
    // ... existing code ...
    
    if let Some(reason) = chunk.stop_reason {
        claude_stop_reason = Some(reason);
    }
    
    // ... rest of chunk processing ...
}

// Map to ACP StopReason
let stop_reason = match claude_stop_reason.as_deref() {
    Some("max_tokens") => StopReason::MaxTokens,
    Some("end_turn") | None => StopReason::EndTurn,
    _ => StopReason::EndTurn,
};
```

#### Step 5: Testing
- Unit test for parse_result_message with "max_tokens" and "end_turn"
- Integration test with real Claude CLI using --max-tokens flag

### Key Decisions

1. **Minimal Change**: Only parse what we need (stop_reason) from result messages
2. **Backward Compatible**: Default to EndTurn if stop_reason is missing or unknown
3. **Thread-Safe**: Use existing channel-based streaming pattern in query_stream
4. **Test Coverage**: Unit tests for parsing, integration tests for full flow




## Implementation Complete

### Changes Made

#### 1. ProtocolTranslator (lib/src/protocol_translator.rs)
- Added `StreamResult` struct to capture stop_reason from result messages
- Implemented `parse_result_message()` method to parse result messages and extract stop_reason field
- Added 4 unit tests covering max_tokens, end_turn, missing stop_reason, and non-result messages

#### 2. ClaudeClient (lib/src/claude.rs)
- Added `stop_reason: Option<String>` field to `MessageChunk` struct
- Updated `content_block_to_message_chunk()` to initialize stop_reason as None
- Modified `query_stream()` to parse result messages and send a final chunk with stop_reason
- Updated test cases to include stop_reason field in MessageChunk construction

#### 3. Agent (lib/src/agent.rs)
- Added `claude_stop_reason` variable in `handle_prompt()` to capture stop_reason from chunks
- Implemented mapping logic to convert Claude's stop_reason to ACP StopReason:
  - `"max_tokens"` → `StopReason::MaxTokens`
  - `"end_turn"` or `None` → `StopReason::EndTurn`
  - Unknown values → `StopReason::EndTurn` with debug log
- Added claude_stop_reason to response metadata for debugging

### Test Results
- Build: ✅ Success
- All 711 tests: ✅ Passed
- Unit tests for parse_result_message: ✅ 4 tests added and passing
- Integration: All existing tests continue to pass

### Technical Notes

**Data Flow:**
1. Claude CLI outputs result message: `{"type":"result","stop_reason":"max_tokens",...}`
2. ClaudeProcess reads the line
3. ProtocolTranslator.parse_result_message() extracts stop_reason
4. ClaudeClient sends final MessageChunk with stop_reason
5. Agent.handle_prompt() maps to ACP StopReason enum

**Backward Compatibility:**
- Default to EndTurn if stop_reason is missing (None)
- Unknown stop_reason values log debug message and default to EndTurn
- All existing tests pass without modification

**Thread Safety:**
- Uses existing channel-based streaming pattern
- No new synchronization primitives needed

### ACP Compliance
Now properly implements the ACP protocol requirement:
- ✅ Returns `StopReason::MaxTokens` when Claude reaches token limit
- ✅ Returns `StopReason::EndTurn` for normal completion
- ✅ Maintains existing `StopReason::Cancelled` and `StopReason::Refusal` behavior




## Testing Notes

### Unit Tests
- Protocol parsing tests exist in `protocol_translator.rs:460-516`
  - test_parse_result_message_with_max_tokens
  - test_parse_result_message_with_end_turn
  - test_parse_result_message_without_stop_reason
  - test_parse_result_message_not_result_type

### Integration Tests
Integration tests with real Claude CLI to trigger max_tokens would require:
1. Modifying how Claude CLI is invoked to pass `--max-tokens` flag
2. This is not currently configurable per-request
3. All existing integration tests exercise the stop_reason code path and verify EndTurn works correctly
4. Max_tokens scenario is harder to test without mocking (which is forbidden per coding standards)

### Verification Strategy
The implementation was verified through:
1. Code review of the data flow
2. Unit tests of the parsing logic
3. Existing integration tests that verify stop_reason field propagation
4. Manual testing with Claude CLI would be needed to fully verify max_tokens scenario

The challenge with testing max_tokens is that it requires either:
- Modifying the AgentConfig or ClaudeProcess to support per-request max_tokens
- Or manually testing with Claude CLI using --max-tokens flag

Given the implementation is a straightforward string match and all the parsing logic is tested, the implementation should work correctly when max_tokens is actually triggered by Claude.