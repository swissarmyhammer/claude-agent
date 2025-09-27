# Replace Custom Notification Types with Agent-Client-Protocol Types

## Problem
The codebase currently defines custom types that duplicate functionality already provided by the `agent-client-protocol` crate. This creates unnecessary complexity and potential inconsistencies with the protocol specification.

## Custom Types to Replace

### 1. SessionUpdateNotification → `agent_client_protocol::SessionNotification`
**Files affected:**
- `lib/src/agent.rs:23`
- `lib/tests/test_client.rs:179`
- `tests/test_client.rs:179`

**Current custom type:**
```rust
pub struct SessionUpdateNotification {
    pub session_id: String,
    pub message_chunk: Option<MessageChunk>,
    pub tool_call_result: Option<ToolCallContent>,
}
```

**Replace with:**
```rust
pub struct SessionNotification {
    pub session_id: SessionId,
    pub update: SessionUpdate,
    pub meta: Option<serde_json::Value>,
}
```

### 2. MessageChunk → `agent_client_protocol::ContentBlock`
**Files affected:**
- `lib/src/agent.rs:50`
- `lib/tests/test_client.rs:186`
- `tests/test_client.rs:186`

**Current custom type:**
```rust
pub struct MessageChunk {
    pub content: Vec<ContentBlock>,
}
```

**Replace with:** Direct use of `agent_client_protocol::ContentBlock` enum

### 3. ToolCallContent → `agent_client_protocol::SessionUpdate::ToolCallUpdate`
**Files affected:**
- `lib/src/agent.rs:37`

**Current custom type:**
```rust
pub struct ToolCallContent {
    pub tool_call_id: String,
    pub result: String,
}
```

**Replace with:** Use `SessionUpdate::ToolCallUpdate(ToolCallUpdate)` variant

## Benefits
1. **Protocol Compliance**: Ensures full compliance with ACP specification
2. **Reduced Complexity**: Eliminates 4+ custom type definitions
3. **Type Safety**: Use `SessionId` instead of raw `String` for session IDs
4. **Consistency**: All session updates use the same structured approach
5. **Maintainability**: Protocol updates automatically benefit our code

## Implementation Tasks
- [ ] Update `lib/src/agent.rs` to use protocol types
- [ ] Update `lib/tests/test_client.rs` to use protocol types
- [ ] Update `tests/test_client.rs` to use protocol types (if needed after test migration)
- [ ] Update all imports throughout the codebase
- [ ] Update function signatures that use the old types
- [ ] Ensure all serialization/deserialization still works correctly
- [ ] Run tests to verify functionality is preserved

## Acceptance Criteria
- All custom notification types are removed
- All functionality continues to work with protocol types
- Tests pass
- Code builds without warnings