# Implement ACP Session Loading with History Replay

## Problem
Our session loading implementation doesn't properly replay conversation history via `session/update` notifications as required by the ACP specification. We need to implement the complete session loading flow with historical message streaming.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/session-setup:

**Session Loading Flow:**
1. Validate agent has `loadSession: true` capability
2. Accept `session/load` request with sessionId, cwd, mcpServers
3. Replay ENTIRE conversation history via `session/update` notifications
4. Send historical messages in original order
5. Respond to `session/load` request ONLY after all history is streamed

**History Replay Example:**
```json
{
  "jsonrpc": "2.0", 
  "method": "session/update",
  "params": {
    "sessionId": "sess_789xyz",
    "update": {
      "sessionUpdate": "user_message_chunk",
      "content": {
        "type": "text",
        "text": "What's the capital of France?"
      }
    }
  }
}
```

Followed by agent response:
```json
{
  "jsonrpc": "2.0",
  "method": "session/update", 
  "params": {
    "sessionId": "sess_789xyz",
    "update": {
      "sessionUpdate": "agent_message_chunk",
      "content": {
        "type": "text",
        "text": "The capital of France is Paris."
      }
    }
  }
}
```

Final response after ALL history streamed:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": null
}
```

## Current Issues
- Session loading may exist but history replay mechanism unclear
- No validation of `loadSession` capability before allowing `session/load`
- Missing proper streaming of historical conversation
- No guarantee of message order during replay
- Response timing may not follow spec (respond only after history complete)

## Implementation Tasks

### Capability Validation
- [ ] Check `loadSession` capability in agent initialization response  
- [ ] Reject `session/load` requests if capability not declared
- [ ] Return proper error for unsupported session loading
- [ ] Add capability checking middleware for `session/load` method

### Session Storage and Retrieval
- [ ] Implement session persistence storage backend
- [ ] Store complete conversation history with metadata
- [ ] Store session configuration (cwd, mcpServers, etc.)
- [ ] Add session lookup by sessionId
- [ ] Handle session expiration and cleanup policies

### History Replay Implementation
- [ ] Retrieve complete conversation history for sessionId
- [ ] Convert stored messages to `session/update` notification format
- [ ] Stream historical messages in chronological order
- [ ] Implement proper message chunking for large conversations
- [ ] Maintain exact original message content and formatting

### Notification Streaming
- [ ] Send `user_message_chunk` notifications for historical user messages
- [ ] Send `agent_message_chunk` notifications for historical agent responses
- [ ] Send `tool_call` and `tool_call_update` notifications for historical tool usage
- [ ] Preserve original message timestamps and metadata
- [ ] Handle different content types (text, images, etc.) in history

### Request Response Flow
- [ ] Accept `session/load` request with proper parameter validation
- [ ] Start history replay before sending response  
- [ ] Stream ALL historical notifications
- [ ] Send `session/load` response ONLY after history replay complete
- [ ] Handle concurrent requests during history replay

### Session State Restoration
- [ ] Restore session working directory from stored config
- [ ] Reconnect to MCP servers specified in load request
- [ ] Restore session context and state
- [ ] Update session metadata with new configuration
- [ ] Handle configuration changes between save and load

## Error Handling
Proper error responses for session loading issues:
```json
{
  "error": {
    "code": -32601,
    "message": "Method not supported: agent does not support loadSession capability",
    "data": {
      "method": "session/load",
      "requiredCapability": "loadSession",
      "declared": false
    }
  }
}
```

```json
{
  "error": {
    "code": -32602,
    "message": "Session not found: sessionId does not exist or has expired", 
    "data": {
      "sessionId": "sess_invalid123",
      "error": "session_not_found"
    }
  }
}
```

## Implementation Notes
Add session loading comments:
```rust
// ACP requires complete conversation history replay during session loading:
// 1. Validate loadSession capability before allowing session/load
// 2. Stream ALL historical messages via session/update notifications
// 3. Maintain exact chronological order of original conversation
// 4. Only respond to session/load AFTER all history is streamed
// 5. Client can then continue conversation seamlessly
```

## Testing Requirements
- [ ] Test capability validation prevents unsupported session loading
- [ ] Test complete conversation history replay in correct order
- [ ] Test different message types in historical replay
- [ ] Test session loading with various conversation lengths
- [ ] Test working directory and MCP server restoration  
- [ ] Test concurrent session operations during history replay
- [ ] Test session loading error scenarios
- [ ] Test session expiration and cleanup
- [ ] Test large conversation history performance

## Acceptance Criteria
- Session loading only available if `loadSession: true` capability declared
- Complete conversation history replayed via `session/update` notifications
- Historical messages streamed in exact chronological order
- All message types properly replayed (user, agent, tool calls, etc.)
- `session/load` response sent only after complete history replay
- Session state fully restored (cwd, MCP servers, context)
- Proper error handling for missing or expired sessions
- Performance optimizations for large conversation histories
- Complete test coverage for all session loading scenarios