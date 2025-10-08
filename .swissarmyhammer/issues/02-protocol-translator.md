# Phase 2: Create Protocol Translator

## Goal
Build the translation layer between ACP protocol and Claude's stream-json format.

## Scope
Create `lib/src/protocol_translator.rs` with:
- ACP ContentBlocks → stream-json (for stdin)
- stream-json → ACP SessionNotifications (from stdout)
- Handle all message types (text, tool calls, tool results)

## Actual Stream-JSON Format

### Input (stdin to claude)
```json
{"type":"user","message":{"role":"user","content":"What is 2+2?"}}
```

### Output (stdout from claude)
```json
{"type":"system","subtype":"init","cwd":"/path","session_id":"uuid","tools":[...],...}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"read_file","input":{...}}],...}}
{"type":"result","subtype":"success","total_cost_usd":0.114,...}
```

## Implementation

### ProtocolTranslator
```rust
pub struct ProtocolTranslator;

impl ProtocolTranslator {
    /// Convert ACP ContentBlocks to stream-json for claude stdin
    pub fn acp_to_stream_json(
        content: Vec<ContentBlock>,
        role: MessageRole,
    ) -> Result<String> {
        // Output: {"type":"user","message":{"role":"user","content":"..."}}
    }
    
    /// Convert stream-json line from claude to ACP SessionNotification
    pub fn stream_json_to_acp(
        line: &str,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        // Parse {"type":"assistant",...} or {"type":"system",...}
        // Return None for {"type":"result"} (metadata only)
    }
    
    /// Handle tool result to stream-json
    pub fn tool_result_to_stream_json(
        tool_call_id: &str,
        result: &str,
    ) -> Result<String> {
        // Output: {"type":"user","message":{"role":"user","content":[{"tool_use_id":"...","type":"tool_result",...}]}}
    }
}
```

## Format Examples

### ACP → stream-json (stdin)

Input (ACP):
```json
{
  "content": [
    {"type": "text", "text": "Hello"}
  ]
}
```

Output (stream-json):
```json
{"type":"user","message":{"role":"user","content":"Hello"}}
```

### stream-json → ACP (stdout)

Input (stream-json):
```json
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}]}}
```

Output (ACP):
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "session-123",
    "update": {
      "type": "agentMessageChunk",
      "content": {"type": "text", "text": "Hi!"}
    }
  }
}
```

### Tool Use Translation

Input (stream-json):
```json
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"read_file","input":{"path":"foo.rs"}}]}}
```

Output (ACP):
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "session-123",
    "update": {
      "type": "agentMessageChunk",
      "content": {
        "type": "toolUse",
        "id": "toolu_123",
        "name": "read_file",
        "input": {"path": "foo.rs"}
      }
    }
  }
}
```

### Tool Result (ACP → stream-json)

After we execute tool locally:
```json
{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_123","type":"tool_result","content":[{"type":"text","text":"file contents"}]}]}}
```

## Message Types to Handle

### From Claude (stdout)
- `type: "system", subtype: "init"` → Save session metadata, don't notify
- `type: "assistant"` with `content: [{"type":"text",...}]` → AgentMessageChunk (text)
- `type: "assistant"` with `content: [{"type":"tool_use",...}]` → AgentMessageChunk (toolUse)
- `type: "result"` → Extract metadata, don't notify

### To Claude (stdin)
- User text → `{"type":"user","message":{"role":"user","content":"..."}}`
- Tool result → `{"type":"user","message":{"role":"user","content":[{"tool_use_id":"...","type":"tool_result","content":[...]}]}}`

## Key Points

1. **We execute tools** - Claude only requests them
2. **Tool results go back as user messages** - not a separate message type
3. **type field is required** in both input and output
4. **Multiple content blocks** possible in tool results

## Error Handling
- Malformed JSON → log and skip line
- Unknown message type → log warning, return None
- Missing required fields → return error

## Testing
- Unit test: translate simple text message ACP → stream-json
- Unit test: translate assistant response stream-json → ACP
- Unit test: translate tool use stream-json → ACP
- Unit test: translate tool result ACP → stream-json
- Unit test: handle malformed JSON gracefully
- Unit test: handle unknown message types

## Acceptance Criteria
- [ ] Can convert ACP ContentBlocks to stream-json
- [ ] Can parse stream-json lines to ACP notifications
- [ ] Handles text messages correctly
- [ ] Handles tool use correctly
- [ ] Handles tool results correctly
- [ ] Handles malformed JSON gracefully
- [ ] All tests pass

## Dependencies
- Depends on: Phase 1 (for integration testing with real processes)
- Uses: `agent_client_protocol` types

## Next Phase
Phase 3: Integration with Agent (separate issue)
