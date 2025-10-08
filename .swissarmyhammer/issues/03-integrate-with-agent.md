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
