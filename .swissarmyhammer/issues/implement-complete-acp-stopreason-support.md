# Implement Complete ACP StopReason Support

## Problem
Our prompt response implementation doesn't include all required stop reasons as specified in the ACP specification. We need complete support for all stop reason types to properly communicate prompt turn completion status.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn:

**Required StopReason Types:**
```json
{
  "result": {
    "stopReason": "end_turn" | "max_tokens" | "max_turn_requests" | "refusal" | "cancelled"
  }
}
```

**Stop Reason Definitions:**
- `end_turn`: Language model finishes responding without requesting more tools
- `max_tokens`: Maximum token limit is reached
- `max_turn_requests`: Maximum number of model requests in a single turn exceeded
- `refusal`: Agent refuses to continue
- `cancelled`: Client cancels the turn

## Current Issues
- `PromptResponse` may not include all stop reason variants
- Missing logic to determine appropriate stop reason
- No token limit or turn request tracking
- Missing refusal detection and handling
- Cancellation stop reason may not be implemented

## Implementation Tasks

### StopReason Type Definition
- [ ] Define complete `StopReason` enum with all required variants
- [ ] Add proper serialization/deserialization for stop reasons
- [ ] Update `PromptResponse` to include stop reason field
- [ ] Add stop reason validation and type safety

### Stop Reason Logic Implementation
- [ ] Implement `end_turn` detection when LM completes without tool requests
- [ ] Add token counting and `max_tokens` detection
- [ ] Implement turn request counting and `max_turn_requests` detection
- [ ] Add refusal detection from language model responses
- [ ] Implement `cancelled` stop reason for client cancellations

### Token Tracking
- [ ] Add token counting throughout prompt turn
- [ ] Track input and output tokens separately
- [ ] Implement configurable token limits
- [ ] Add token limit checking before LM requests
- [ ] Return `max_tokens` when limit exceeded

### Turn Request Tracking
- [ ] Count number of LM requests in single turn
- [ ] Implement configurable turn request limits
- [ ] Check limits before each LM request
- [ ] Return `max_turn_requests` when limit exceeded

### Refusal Detection
- [ ] Detect when language model refuses to respond
- [ ] Identify refusal patterns in LM responses
- [ ] Handle safety/policy refusals appropriately
- [ ] Return `refusal` stop reason with proper context

### Cancellation Integration
- [ ] Set `cancelled` stop reason when client cancels
- [ ] Ensure cancellation stops all ongoing operations
- [ ] Handle partial completion scenarios during cancellation
- [ ] Coordinate with cancellation notification system

## Error Handling and Edge Cases
- [ ] Handle scenarios where multiple stop conditions occur
- [ ] Prioritize stop reasons appropriately
- [ ] Add proper logging for stop reason determination
- [ ] Handle edge cases in token/request counting

## Implementation Notes
Add stop reason logic comments:
```rust
// ACP requires specific stop reasons for all prompt turn completions:
// 1. end_turn: Normal completion without pending operations
// 2. max_tokens: Token limit exceeded (configurable)
// 3. max_turn_requests: Too many LM requests in single turn
// 4. refusal: Language model or agent refuses to continue
// 5. cancelled: Client explicitly cancelled the operation
//
// Stop reason must accurately reflect why the turn ended.
```

## Configuration Support
- [ ] Add configurable token limits per session/agent
- [ ] Add configurable turn request limits
- [ ] Support different limits based on session type
- [ ] Add runtime limit adjustment capabilities

## Response Format Implementation
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StopReason {
    #[serde(rename = "end_turn")]
    EndTurn,
    #[serde(rename = "max_tokens")]
    MaxTokens,
    #[serde(rename = "max_turn_requests")]
    MaxTurnRequests,
    #[serde(rename = "refusal")]
    Refusal,
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResponse {
    pub stop_reason: StopReason,
    // Additional metadata fields...
}
```

## Testing Requirements
- [ ] Test all stop reason types are properly returned
- [ ] Test token limit enforcement with `max_tokens` response
- [ ] Test turn request limit enforcement
- [ ] Test refusal detection and stop reason
- [ ] Test cancellation integration with `cancelled` stop reason
- [ ] Test stop reason prioritization in edge cases
- [ ] Test configuration of limits affects stop reason behavior

## Integration Points
- [ ] Connect to language model integration for token counting
- [ ] Integrate with cancellation notification system
- [ ] Connect to tool execution system for turn completion
- [ ] Integrate with session management for limit configuration

## Acceptance Criteria
- Complete `StopReason` enum with all ACP-required variants
- Accurate stop reason determination for all prompt turn endings
- Token counting and limit enforcement with `max_tokens` stop reason
- Turn request counting and limit enforcement
- Refusal detection from language model responses
- Cancellation integration with proper `cancelled` stop reason
- Configurable limits for tokens and turn requests
- Comprehensive test coverage for all stop reason scenarios
- Proper integration with existing prompt processing system