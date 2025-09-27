# Replace Custom Claude Types with claude-sdk-rs Types

## Problem
The `lib/src/claude.rs` module defines custom types that duplicate or overlap with functionality provided by the `claude-sdk-rs` crate. This creates unnecessary complexity, reduces type safety, and prevents leveraging the SDK's built-in capabilities like cost tracking, rich metadata, and proper session management.

## Custom Types to Replace

### 1. MessageRole → `claude_sdk_rs::MessageType`
**Files affected:**
- `lib/src/claude.rs:30`

**Current custom type:**
```rust
pub enum MessageRole {
    User,
    Assistant,
    System,
}
```

**Replace with:**
```rust
pub enum MessageType {
    User,
    Assistant,
    System,
    Init,
    Result,
    Tool,
    ToolResult,
}
```

**Benefits:** SDK type includes tool-related message types and is more comprehensive.

### 2. ClaudeMessage → `claude_sdk_rs::Message`
**Files affected:**
- `lib/src/claude.rs:25`

**Current custom type:**
```rust
pub struct ClaudeMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: SystemTime,
}
```

**Replace with:**
```rust
pub enum Message {
    User { content: String, meta: MessageMeta },
    Assistant { content: String, meta: MessageMeta },
    System { content: String, meta: MessageMeta },
    // ... other variants
}
```

**Benefits:** SDK Message includes rich metadata (cost tracking, token usage, session ID, processing duration).

### 3. MessageChunk → Leverage `claude_sdk_rs::Message` streaming
**Files affected:**
- `lib/src/claude.rs:41`

**Current custom type:**
```rust
pub struct MessageChunk {
    pub content: String,
    pub chunk_type: ChunkType,
}
```

**Replace with:** Use SDK's built-in streaming capabilities and Message variants.

### 4. ChunkType → Use `claude_sdk_rs::MessageType`
**Files affected:**
- `lib/src/claude.rs:47`

**Current custom type:**
```rust
pub enum ChunkType {
    Text,
    ToolCall,
    ToolResult,
}
```

**Replace with:** Direct use of `MessageType` enum which covers these cases.

### 5. SessionContext → `claude_sdk_rs::Session` and related types
**Files affected:**
- `lib/src/claude.rs:17`

**Current custom type:**
```rust
pub struct SessionContext {
    pub session_id: String,
    pub messages: Vec<ClaudeMessage>,
    pub created_at: SystemTime,
}
```

**Replace with:** Use SDK's `Session`, `SessionManager`, and `SessionId` types.

## Additional SDK Benefits to Leverage

### Rich Metadata (`MessageMeta`)
- **Cost tracking**: `cost_usd` field for financial monitoring
- **Token usage**: `TokenUsage` struct with input/output/total counts
- **Processing duration**: `duration_ms` for performance monitoring
- **Session correlation**: `session_id` for proper session management

### Type Safety
- **SessionId**: Type-safe session identifiers instead of raw strings
- **Comprehensive enums**: More complete coverage of message types

### Session Management
- **SessionManager**: Proper session lifecycle management
- **Session**: Full session state with metadata
- **SessionStorage**: Persistent session storage capabilities

## Benefits of This Cleanup
1. **Reduced Code Complexity**: Eliminates 5+ custom type definitions
2. **Enhanced Functionality**: Access to cost tracking, token usage, and rich metadata
3. **Type Safety**: Use `SessionId` instead of raw `String` for session IDs
4. **Better Integration**: Seamless integration with SDK streaming and session management
5. **Maintainability**: SDK updates automatically benefit our code
6. **Consistency**: All Claude interactions use the same structured approach

## Implementation Tasks
- [ ] Update `lib/src/claude.rs` to import and use SDK types
- [ ] Replace `MessageRole` with `claude_sdk_rs::MessageType`
- [ ] Replace `ClaudeMessage` with `claude_sdk_rs::Message`
- [ ] Replace `MessageChunk` and `ChunkType` with SDK streaming approach
- [ ] Replace `SessionContext` with `claude_sdk_rs::Session`
- [ ] Update all From/Into implementations to work with SDK types
- [ ] Update function signatures throughout the codebase
- [ ] Update imports in dependent modules
- [ ] Leverage `MessageMeta` for rich metadata where beneficial
- [ ] Run tests to verify functionality is preserved
- [ ] Update any serialization/deserialization code

## Acceptance Criteria
- All custom Claude types are removed in favor of SDK types
- Rich metadata (cost, tokens, duration) is properly utilized
- Session management uses type-safe `SessionId`
- All functionality continues to work with SDK types
- Tests pass
- Code builds without warnings
- Streaming responses work properly with SDK Message types