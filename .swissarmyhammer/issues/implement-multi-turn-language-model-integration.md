# Implement Multi-turn Language Model Integration

## Problem
Our prompt processing doesn't properly implement the multi-turn language model integration required by the ACP specification. After tool execution completes, results should be sent back to the language model to continue the conversation until completion.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn:

**Multi-turn Flow:**
1. Send user prompt to language model
2. LM responds with text and/or tool calls
3. Execute tools and collect results
4. Send tool results back to LM as next request
5. Repeat until LM completes response without requesting tools
6. Return final response with appropriate stop reason

**The specification states:** "The Agent sends the tool results back to the language model as another request. The cycle returns to step 2, continuing until the language model completes its response without requesting additional tool calls."

## Current Issues
- Tool results may not be properly sent back to language model
- Missing multi-turn conversation flow with LM
- No handling of LM responses that include both text and tool calls
- Missing integration between tool execution and LM continuation
- No proper turn completion detection

## Implementation Tasks

### Language Model Conversation Management
- [ ] Implement conversation history tracking for multi-turn interactions
- [ ] Add tool result formatting for LM consumption
- [ ] Handle mixed LM responses (text + tool calls)
- [ ] Implement proper conversation state management
- [ ] Add turn boundary detection and management

### Tool Result Integration
- [ ] Format tool execution results for language model input
- [ ] Include tool call context and metadata in LM requests
- [ ] Handle different types of tool outputs (text, structured data, errors)
- [ ] Implement tool result validation before sending to LM
- [ ] Support tool result summarization for large outputs

### Multi-turn Request Flow
- [ ] Send initial user prompt to language model
- [ ] Process LM response for text and tool call requests
- [ ] Execute requested tools and collect results
- [ ] Format tool results and send back to LM
- [ ] Continue conversation until LM completion
- [ ] Detect when LM has no more tool requests

### LM Response Processing
- [ ] Parse LM responses for text content and tool calls
- [ ] Handle streaming LM responses properly
- [ ] Extract tool call requests with parameters
- [ ] Process mixed responses with both content and tool calls
- [ ] Validate LM tool call requests against available tools

### Turn Completion Detection
- [ ] Detect when LM completes without requesting tools
- [ ] Implement proper `end_turn` stop reason logic
- [ ] Handle LM refusal scenarios
- [ ] Track token usage across multiple LM requests
- [ ] Implement turn request limits and enforcement

## Conversation Flow Implementation
```rust
pub struct ConversationManager {
    messages: Vec<LmMessage>,
    pending_tool_calls: HashMap<String, ToolCall>,
    turn_count: u32,
    token_usage: TokenUsage,
}

impl ConversationManager {
    pub async fn process_turn(&mut self, user_input: &str) -> Result<PromptResponse>;
    pub async fn send_to_lm(&self, messages: &[LmMessage]) -> Result<LmResponse>;
    pub fn add_tool_results(&mut self, results: Vec<ToolResult>);
    pub fn is_turn_complete(&self, response: &LmResponse) -> bool;
}
```

## Implementation Notes
Add multi-turn LM integration comments:
```rust
// ACP requires complete multi-turn conversation flow:
// 1. Send user prompt to language model
// 2. Process LM response for text and tool calls
// 3. Execute tools and collect results  
// 4. Send tool results back to LM for continuation
// 5. Repeat until LM completes without tool requests
// 6. Return final response with appropriate stop reason
//
// Each turn may involve multiple LM requests with tool results.
```

### Language Model Integration Points
- [ ] Connect to Claude SDK or language model API
- [ ] Handle LM API rate limits and errors
- [ ] Implement proper authentication and configuration
- [ ] Support different LM models and capabilities
- [ ] Handle LM streaming responses appropriately

### Tool Call Coordination
- [ ] Extract tool call requests from LM responses
- [ ] Execute tools with proper parameter parsing
- [ ] Collect and format tool execution results
- [ ] Handle tool execution errors and failures
- [ ] Support concurrent tool execution where safe

### Session Update Integration
- [ ] Send `agent_message_chunk` updates for LM text responses
- [ ] Send `tool_call` updates when LM requests tools
- [ ] Coordinate tool status updates with LM conversation
- [ ] Handle streaming updates during multi-turn conversation

### Error Handling and Recovery
- [ ] Handle LM API errors during multi-turn conversation
- [ ] Recover from tool execution failures gracefully
- [ ] Handle malformed LM responses
- [ ] Implement fallback strategies for LM failures
- [ ] Support conversation recovery after transient failures

### Performance and Resource Management
- [ ] Optimize conversation history management
- [ ] Handle large tool outputs efficiently
- [ ] Implement conversation length limits
- [ ] Add memory management for long conversations
- [ ] Support conversation checkpointing

## Testing Requirements
- [ ] Test basic single-turn conversation (no tools)
- [ ] Test multi-turn conversation with tool execution
- [ ] Test mixed LM responses with text and tool calls
- [ ] Test tool result formatting and LM integration
- [ ] Test turn completion detection
- [ ] Test error handling during multi-turn conversation
- [ ] Test token and turn limit enforcement
- [ ] Test concurrent tool execution integration

## Integration Points
- [ ] Connect to existing tool execution system
- [ ] Integrate with session update notification system
- [ ] Connect to cancellation system for turn cancellation
- [ ] Integrate with stop reason determination logic

## Configuration and Limits
- [ ] Add configurable turn limits per conversation
- [ ] Support token limits across multi-turn conversations
- [ ] Configure tool execution timeouts
- [ ] Add LM request retry policies
- [ ] Support conversation length management

## Acceptance Criteria
- Complete multi-turn conversation flow implemented
- Tool results properly formatted and sent back to language model
- LM responses processed for both text content and tool calls
- Turn completion detected when LM finishes without tool requests
- Proper stop reason determination (`end_turn`, `max_tokens`, etc.)
- Integration with existing tool execution and status reporting
- Error handling and recovery for LM and tool failures
- Performance optimization for long multi-turn conversations
- Comprehensive test coverage for all conversation scenarios
- Configuration support for limits and policies