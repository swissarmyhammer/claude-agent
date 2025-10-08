# Phase 3: Integrate Process Manager and Translator with Agent

## Goal
Wire up ClaudeProcessManager and ProtocolTranslator into the existing Agent/ClaudeClient architecture.

## Scope
- Update `lib/src/claude.rs` to use ProcessManager instead of SDK
- Hook into `lib/src/agent.rs` session lifecycle
- Update prompt handling to use translator
- Keep public API compatible

## Implementation

### Update ClaudeClient (lib/src/claude.rs)

```rust
pub struct ClaudeClient {
    process_manager: Arc<ClaudeProcessManager>,
    translator: ProtocolTranslator,
}

impl ClaudeClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            process_manager: Arc::new(ClaudeProcessManager::new()),
            translator: ProtocolTranslator,
        })
    }
    
    pub async fn query_stream_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> Result<impl Stream<Item = MessageChunk>> {
        let session_id = &context.session_id;
        
        // Get or spawn process for this session
        let process = self.process_manager.get_or_spawn(session_id).await?;
        
        // Translate prompt to stream-json
        let stream_json = self.translator.acp_to_stream_json(
            vec![ContentBlock::Text(TextContent { text: prompt.to_string(), ..Default::default() })],
            MessageRole::User
        )?;
        
        // Write to claude stdin
        let mut process = process.lock().await;
        process.write_line(&stream_json).await?;
        
        // Create stream that reads from stdout and translates
        let stream = self.create_message_stream(process, session_id);
        
        Ok(stream)
    }
    
    async fn create_message_stream(
        &self,
        process: Arc<Mutex<ClaudeProcess>>,
        session_id: &SessionId,
    ) -> impl Stream<Item = MessageChunk> {
        // Read lines from stdout, translate to MessageChunk
    }
}
```

### Hook into Agent Session Lifecycle (lib/src/agent.rs)

```rust
impl ClaudeAgent {
    async fn handle_session_create(&self, params: SessionParams) -> Result<SessionInfo> {
        let session_id = params.session_id;
        
        // Spawn persistent claude process for this session
        self.claude_client.process_manager
            .spawn_for_session(session_id.clone())
            .await?;
        
        // ... rest of session creation
    }
    
    async fn handle_session_delete(&self, session_id: &SessionId) -> Result<()> {
        // Terminate claude process
        self.claude_client.process_manager
            .terminate_session(session_id)
            .await?;
        
        // ... rest of session cleanup
    }
}
```

### Update Prompt Handling

```rust
async fn handle_prompt(&self, params: PromptParams) -> Result<PromptResponse> {
    let session_id = &params.session_id;
    let session = self.session_manager.get_session(session_id)?;
    
    // Convert to SessionContext
    let context: crate::claude::SessionContext = session.into();
    
    // Stream from claude process (now uses ProcessManager internally)
    let mut stream = self.claude_client
        .query_stream_with_context(&prompt_text, &context)
        .await?;
    
    // Rest is same - stream chunks and send notifications
    while let Some(chunk) = stream.next().await {
        let notification = /* create ACP notification from chunk */;
        self.send_session_update(notification).await?;
    }
    
    Ok(PromptResponse { ... })
}
```

## Changes Required

### lib/src/claude.rs
- Remove SDK Client usage
- Add `process_manager: Arc<ClaudeProcessManager>`
- Update `query_stream_with_context()` to use ProcessManager
- Keep `SessionContext`, `MessageChunk` types unchanged
- Update error handling

### lib/src/agent.rs
- Hook session/create → spawn process
- Hook session/delete → terminate process
- Update prompt handling to work with new ClaudeClient
- No changes to ACP protocol handling

### lib/src/error.rs
- Remove `Claude(claude_sdk_rs::Error)` variant
- Add `ProcessError(String)` for process management errors
- Add `ProtocolError(String)` for translation errors

## Migration Strategy

1. Keep existing code working while adding new code
2. Add ProcessManager to ClaudeClient alongside SDK client
3. Add feature flag to toggle between old/new implementation
4. Test new implementation thoroughly
5. Remove SDK code in next phase

## Testing
- Integration test: full session lifecycle with real claude
- Integration test: multiple concurrent sessions
- Integration test: process crash recovery
- Unit test: session create/delete hooks
- Existing tests should continue to pass

## Acceptance Criteria
- [ ] ClaudeClient uses ProcessManager instead of SDK
- [ ] Session creation spawns claude process
- [ ] Session deletion terminates claude process
- [ ] Prompts route through translator and process
- [ ] Multiple sessions work concurrently
- [ ] Existing tests pass (or updated)
- [ ] Integration tests pass with real claude CLI

## Dependencies
- Depends on: Phase 1 (ClaudeProcessManager)
- Depends on: Phase 2 (ProtocolTranslator)

## Next Phase
Phase 4: Remove SDK dependency (separate issue)



## Proposed Solution

After analyzing the existing code, I've identified the integration strategy:

### Key Observations
1. **ClaudeProcessManager exists** at `lib/src/claude_process.rs` - manages persistent claude CLI processes per session
2. **ProtocolTranslator exists** at `lib/src/protocol_translator.rs` - translates between ACP and stream-json formats
3. **Current ClaudeClient** uses `claude-sdk-rs` with `Client` struct and `MessageStream`
4. **Current architecture** has ClaudeClient completely separate from session lifecycle in agent.rs

### Implementation Plan

#### Step 1: Update Error Types (error.rs)
- Remove `Claude(#[from] claude_sdk_rs::Error)` variant
- Add process and protocol error variants:
  - `ProcessError(String)` for process management failures
  - `ProtocolError` already exists, ensure it covers translation errors

#### Step 2: Refactor ClaudeClient (claude.rs)
Replace SDK-based implementation with process-based:

```rust
pub struct ClaudeClient {
    process_manager: Arc<ClaudeProcessManager>,
}

impl ClaudeClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            process_manager: Arc::new(ClaudeProcessManager::new()),
        })
    }
    
    // Keep existing method signatures for compatibility
    pub async fn query_stream_with_context(
        &self,
        prompt: &str,
        context: &SessionContext,
    ) -> Result<impl Stream<Item = MessageChunk>> {
        // Get process for session
        let process = self.process_manager.get_process(&context.session_id).await?;
        
        // Build conversation from context
        let full_conversation = self.build_conversation(prompt, context);
        
        // Translate to stream-json
        let content = vec![ContentBlock::Text(TextContent {
            text: full_conversation,
            annotations: None,
            meta: None,
        })];
        let stream_json = ProtocolTranslator::acp_to_stream_json(content)?;
        
        // Write to process
        let mut proc = process.lock().unwrap();
        proc.write_line(&stream_json).await?;
        drop(proc);
        
        // Create stream that reads from stdout
        let stream = self.create_message_stream(process, &context.session_id);
        Ok(stream)
    }
    
    fn create_message_stream(
        &self,
        process: Arc<Mutex<ClaudeProcess>>,
        session_id: &SessionId,
    ) -> impl Stream<Item = MessageChunk> {
        // Use async_stream to read lines and translate
        async_stream::stream! {
            loop {
                let line = {
                    let mut proc = process.lock().unwrap();
                    proc.read_line().await
                };
                
                match line {
                    Ok(Some(line)) => {
                        // Translate stream-json to ACP
                        if let Ok(Some(notification)) = 
                            ProtocolTranslator::stream_json_to_acp(&line, session_id) {
                            // Convert SessionNotification to MessageChunk
                            if let Some(chunk) = Self::notification_to_chunk(notification) {
                                yield chunk;
                            }
                        }
                    }
                    Ok(None) => break, // EOF
                    Err(_) => break,
                }
            }
        }
    }
    
    fn notification_to_chunk(notification: SessionNotification) -> Option<MessageChunk> {
        match notification.update {
            SessionUpdate::AgentMessageChunk { content } => {
                Some(MessageChunk {
                    content: Self::content_block_to_string(&content),
                    chunk_type: ChunkType::Text,
                    tool_call: None,
                    token_usage: None,
                    meta: None,
                })
            }
            _ => None,
        }
    }
}
```

#### Step 3: Hook Session Lifecycle (agent.rs)
Need to read agent.rs to find session create/delete handlers and add process management:

```rust
// In session/create handler
async fn handle_session_create(&self, params: SessionParams) -> Result<SessionInfo> {
    // Spawn claude process for this session
    self.claude_client.process_manager
        .spawn_for_session(params.session_id).await?;
    
    // ... rest of session creation
}

// In session/delete handler  
async fn handle_session_delete(&self, session_id: &SessionId) -> Result<()> {
    // Terminate claude process
    self.claude_client.process_manager
        .terminate_session(session_id).await?;
    
    // ... rest of session cleanup
}
```

#### Step 4: Handle Dependencies
- Add `async-stream` crate for stream creation
- Ensure `agent-client-protocol` types are available
- Keep public API compatible (SessionContext, MessageChunk, etc.)

### Challenges Identified
1. **MessageChunk conversion**: SDK's Message enum is different from our MessageChunk - need careful mapping
2. **Error handling**: stream-json may have different error patterns than SDK
3. **Metadata extraction**: SDK provides cost/tokens in MessageMeta, stream-json has different format
4. **Testing**: Need to ensure existing tests still pass with process-based implementation

### Testing Strategy
1. Run existing unit tests in claude.rs - some will need updates
2. Integration tests should work if public API is preserved
3. Add new test for process lifecycle integration



## Implementation Notes

### Code Review Fixes Completed

Successfully refactored `lib/src/claude.rs` to remove SDK dependency and use ProcessManager:

1. **Removed SDK References**
   - Removed `claude_sdk_rs::Error` type usage
   - Removed `Message`, `MessageMeta` SDK enum types
   - Removed `execute_with_retry()` dead code
   - Removed `is_retryable()` dead code

2. **Implemented Process-Based Methods**
   - Reimplemented `query()` using ProcessManager
   - Reimplemented `query_stream()` using ProcessManager with tokio channels
   - Reimplemented `query_with_context()` delegating to `query()`
   - Reimplemented `query_stream_with_context()` delegating to `query_stream()`

3. **Added Helper Methods**
   - `to_acp_session_id()` - converts between session::SessionId and ACP SessionId
   - `content_block_to_message_chunk()` - converts ContentBlock to MessageChunk
   - `send_prompt_to_process()` - extracts duplicated prompt-sending logic
   - `is_end_of_stream()` - proper JSON parsing for stream termination detection

4. **Fixed Compilation Issues**
   - Handled SessionId type mismatch between crate and ACP protocol
   - Fixed async Mutex issue by using tokio channel-based streaming
   - Used `tokio::task::spawn_blocking` to handle std::sync::Mutex in async context
   - Removed unused imports

5. **Updated Data Structures**
   - Removed `meta` field references from tests
   - Simplified `add_message()` to remove SDK metadata handling
   - Updated test `test_session_context_token_tracking` to work without SDK types

### Build Status

✅ **Compilation**: PASSES
- All SDK types successfully removed
- No compilation errors
- No warnings (except one unused import that was fixed)

### Test Status

❌ **Tests**: 1 FAILING
- `agent::tests::test_conversation_context_maintained` fails with "Internal error"
- This test makes real calls to Claude process
- Likely related to process spawning or session management integration
- 57 other tests pass successfully

### Architecture Changes

The new architecture eliminates SDK dependency:
```
Old: ClaudeClient → claude-sdk-rs → HTTP API
New: ClaudeClient → ClaudeProcessManager → claude CLI process → stream-json
```

Key benefits:
- Direct process management per session
- Streaming via process stdout/stdin
- Better control over process lifecycle
- Eliminates HTTP overhead

### Remaining Work

1. **Fix failing test**: The `test_conversation_context_maintained` test needs investigation
   - May need to spawn process before first query
   - May need session lifecycle hooks in agent.rs
   
2. **Session lifecycle integration** (agent.rs): Not yet implemented
   - Need to hook session/create to spawn process
   - Need to hook session/delete to terminate process

3. **Integration testing**: Need real claude CLI tests
   - Test multiple concurrent sessions
   - Test process crash recovery
   - Test full session lifecycle

### Technical Decisions

**Stream Implementation**: Used tokio channel-based approach instead of async_stream
- Reason: std::sync::Mutex not compatible with async across await points
- Solution: spawn_blocking task reads from process, sends chunks via channel
- Trade-off: Small overhead for channel communication, but ensures Send safety

**SessionId Conversion**: Added explicit conversion helper
- Two different SessionId types (crate vs ACP)
- Conversion: crate::session::SessionId.to_string() → Arc<str> → ACP SessionId

**Error Handling**: Simplified to use AgentError::Process
- Removed dependency on SDK error types
- Process errors now use string messages
- Can be enhanced later with structured error types
