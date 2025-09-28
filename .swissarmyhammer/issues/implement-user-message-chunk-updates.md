# Implement User Message Chunk Updates

## Problem
Our prompt processing doesn't send user message chunk updates via `session/update` notifications as required by the ACP specification. Clients should receive echoed user input during prompt processing for transparency and UI updates.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn and related specifications:

**User Message Chunk Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/update", 
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "user_message_chunk",
      "content": {
        "type": "text",
        "text": "Can you analyze this code for potential issues?"
      }
    }
  }
}
```

**Purpose:**
- Echo user input back to client during prompt processing
- Enable client UI to display user messages in conversation flow
- Provide consistency with agent message chunk reporting
- Support conversation history visualization

## Current Issues
- No user message chunk updates sent during prompt processing
- User input not echoed back to client via session updates
- Missing conversation flow transparency for clients
- Inconsistent message reporting (agent chunks but no user chunks)

## Implementation Tasks

### User Message Processing Integration
- [ ] Add user message chunk sending to prompt processing flow
- [ ] Extract content blocks from user prompt for echoing
- [ ] Send user message chunks before agent processing begins
- [ ] Support different content types in user message chunks

### Content Block Processing
- [ ] Process user prompt content blocks for echoing
- [ ] Support text, image, audio, resource, and resource_link content
- [ ] Handle content block validation during user message processing
- [ ] Add content block conversion for session update format

### Session Update Integration
- [ ] Integrate user message chunks with existing session update system
- [ ] Ensure proper ordering of user chunks before agent chunks
- [ ] Support user message chunking for large prompts
- [ ] Add user message update correlation with session state

### Prompt Flow Enhancement
- [ ] Send user message chunks at start of prompt processing
- [ ] Maintain conversation flow visibility for clients
- [ ] Support conversation history reconstruction from updates
- [ ] Add user message timestamp and metadata

## User Message Implementation
```rust
impl ClaudeAgent {
    async fn process_prompt_with_user_echo(
        &self,
        session_id: &SessionId,
        prompt: &[ContentBlock],
    ) -> crate::Result<PromptResponse> {
        // Send user message chunks for each content block
        for content_block in prompt {
            self.send_session_update(SessionNotification {
                session_id: session_id.clone(),
                update: SessionUpdate::UserMessageChunk {
                    content: content_block.clone(),
                },
                meta: None,
            }).await?;
        }
        
        // Continue with normal prompt processing
        self.process_prompt_internal(session_id, prompt).await
    }
}
```

## Implementation Notes
Add user message chunk comments:
```rust
// ACP requires user message chunk updates for conversation transparency:
// 1. Echo user input via session/update with user_message_chunk
// 2. Send before agent processing begins
// 3. Include all content blocks from user prompt
// 4. Maintain conversation flow visibility for clients
// 5. Support conversation history reconstruction
//
// User message chunks provide consistent conversation reporting.
```

### Content Processing for User Messages
- [ ] Handle different content types in user message chunks
- [ ] Validate content blocks during user message processing
- [ ] Support content block streaming for large user inputs
- [ ] Add content block metadata preservation

### Conversation Flow Coordination
```rust
impl ConversationFlowManager {
    pub async fn process_user_prompt(
        &self,
        session_id: &SessionId,
        prompt: &[ContentBlock],
    ) -> Result<(), ConversationError> {
        // Send user message chunks
        for (index, content_block) in prompt.iter().enumerate() {
            self.send_user_message_chunk(session_id, content_block, index).await?;
        }
        
        // Add delay to ensure proper message ordering
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        Ok(())
    }
}
```

### Message Ordering and Timing
- [ ] Ensure user message chunks are sent before agent processing
- [ ] Add proper message ordering and sequencing
- [ ] Support conversation flow timing and pacing
- [ ] Handle concurrent user message and agent response coordination

### Error Handling
- [ ] Handle user message chunk sending failures gracefully
- [ ] Continue prompt processing if user echo fails
- [ ] Add logging for user message chunk errors
- [ ] Support partial user message echo scenarios

## Testing Requirements
- [ ] Test user message chunks sent for all prompt requests
- [ ] Test user message chunks include all content blocks from prompt
- [ ] Test proper ordering of user chunks before agent chunks
- [ ] Test different content types in user message chunks
- [ ] Test user message chunk integration with existing session updates
- [ ] Test error handling for user message chunk failures
- [ ] Test conversation flow reconstruction from session updates

## Integration Points
- [ ] Connect to existing prompt processing system
- [ ] Integrate with session update notification system
- [ ] Connect to content block processing and validation
- [ ] Integrate with conversation management and history

## Performance Considerations
- [ ] Optimize user message chunk processing for large prompts
- [ ] Support efficient content block iteration and processing
- [ ] Add user message chunk batching where appropriate
- [ ] Monitor performance impact of additional session updates

## Acceptance Criteria
- User message chunks sent via session/update for all prompts
- All content blocks from user prompt included in chunks
- Proper message ordering with user chunks before agent processing
- Support for all ACP content types in user message chunks
- Integration with existing session update and notification systems
- Error handling allows prompt processing to continue if echo fails
- Comprehensive test coverage for user message chunk scenarios
- Performance optimization for user message processing overhead