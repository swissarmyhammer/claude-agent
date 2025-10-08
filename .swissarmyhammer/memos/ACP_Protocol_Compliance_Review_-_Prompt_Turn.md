# ACP Protocol Compliance Review - Prompt Turn

## Protocol Requirements vs Implementation

### ✅ Compliant Areas

#### 1. Session/Prompt Request Handling
- **Spec**: Client sends `session/prompt` with sessionId and prompt content
- **Implementation**: ✅ Handled in `handle_prompt()` at agent.rs:1200+
- **Validation**: ✅ Content capability validation at line 1255-1273

#### 2. Agent Processing & Streaming
- **Spec**: Agent processes message and sends to language model
- **Implementation**: ✅ Uses `query_stream_with_context()` at line 1296-1303
- **Streaming**: ✅ Streams chunks via `session/update` notifications at line 1335-1357

#### 3. Session Update Notifications  
- **Spec**: Agent sends `session/update` with `AgentMessageChunk`
- **Implementation**: ✅ Proper SessionNotification structure at line 1336-1346
- **Content**: ✅ Uses ContentBlock::Text with TextContent

#### 4. Stop Reasons
- **Spec**: Must respond with appropriate StopReason
- **Implementation**: ✅ Uses:
  - `StopReason::EndTurn` for normal completion (line 1435)
  - `StopReason::MaxTokens` for token limit reached (line 1434) - ✅ IMPLEMENTED 2025-10-08
  - `StopReason::Cancelled` for cancellations (lines 1323, 1377)
  - `StopReason::Refusal` for refusals (line 1388 via create_refusal_response)

#### 5. Refusal Detection
- **Spec**: Agent must detect and report refusals
- **Implementation**: ✅ Comprehensive refusal pattern matching in `is_response_refusal()` at line 1697
- **Patterns**: Detects "I can't", "I cannot", "I'm unable to", "I won't", etc.
- **Logic**: Smart detection - only in short responses (<200 chars) or at start of longer ones

#### 6. Cancellation Handling
- **Spec**: Client sends `session/cancel`, Agent must stop operations and respond with `cancelled` stop reason
- **Implementation**: ✅ 
  - Handles `session/cancel` notification at line 2880+
  - Marks session as cancelled at line 2896
  - Checks cancellation during streaming (lines 1311-1328, 1361-1377)
  - Returns `StopReason::Cancelled` properly

### ⚠️ Areas to Verify

#### 1. Tool Call Lifecycle
- **Spec**: Agent reports tool calls via `session/update` with `tool_call` update
- **Implementation**: Need to verify if we send proper tool_call updates
- **Status**: Tools are executed but need to check if we send:
  - Initial `tool_call` notification with status "pending"
  - Update with status "in_progress" 
  - Final update with status "completed" and content
- **Location**: Check tool execution in tool handler

#### 2. Permission Requests
- **Spec**: Agent MAY request permission via `session/request_permission` before tool execution
- **Implementation**: Need to verify permission request handling
- **Status**: Unknown - need to check if permission requests are sent before tool calls

#### 3. Multiple Turn Requests
- **Spec**: Agent stops with `max_turn_requests` if too many model requests in single turn
- **Implementation**: ⚠️ NOT IMPLEMENTED
- **Missing**: No counter for model requests per turn
- **Missing**: No `StopReason::MaxTurnRequests` usage

#### 4. Token Limit Handling
- **Spec**: Agent stops with `max_tokens` if token limit reached
- **Implementation**: ✅ IMPLEMENTED 2025-10-08
- **Details**:
  - Parses stop_reason from Claude result messages (protocol_translator.rs:206)
  - Maps "max_tokens" → StopReason::MaxTokens (agent.rs:1434)
  - Includes claude_stop_reason in response metadata (agent.rs:1449)
  - Data flow: ClaudeProcess → ProtocolTranslator → ClaudeClient → Agent
- **Test Coverage**: Unit tests in protocol_translator.rs:460-516

### ❌ Deviations Found

#### 1. Missing Stop Reasons
**Issue**: We don't handle all required stop reasons

**Status Update (2025-10-08)**:
- ✅ `StopReason::MaxTokens` - IMPLEMENTED (see issue acp-implement-max-tokens-stop-reason)
- ❌ `StopReason::MaxTurnRequests` - NOT YET IMPLEMENTED (see issue acp-implement-max-turn-requests-stop-reason)

**Completed for MaxTokens**:
1. ✅ Parse Claude's stop_reason from result messages (protocol_translator.rs:206)
2. ✅ Map Claude's "max_tokens" → StopReason::MaxTokens (agent.rs:1434)
3. ✅ Capture stop_reason during streaming (agent.rs:1333-1335)
4. ✅ Return appropriate stop reason with metadata (agent.rs:1442-1451)

**Still Required for MaxTurnRequests**:
1. Add turn request counter per prompt turn
2. Check against max_turn_requests limit from config
3. Return StopReason::MaxTurnRequests when limit exceeded

#### 2. Tool Call Status Reporting
**Issue**: Need to verify we send all three status updates per tool call

**Spec Requires**:
1. Initial: `{"sessionUpdate": "tool_call", "toolCallId": "...", "status": "pending"}`
2. Start: `{"sessionUpdate": "tool_call_update", "toolCallId": "...", "status": "in_progress"}`
3. Complete: `{"sessionUpdate": "tool_call_update", "toolCallId": "...", "status": "completed", "content": [...]}`

**Action**: Check tool call handler to verify these are sent

## Recommendations

### High Priority
1. ✅ **Refusal Detection**: Already implemented comprehensively
2. ✅ **Cancellation**: Already implemented correctly
3. ✅ **MaxTokens Stop Reason**: COMPLETED 2025-10-08
4. ❌ **MaxTurnRequests Stop Reason**: Still needs implementation
5. ⚠️ **Tool Status**: Verify tool_call status updates are sent

### Medium Priority
1. ✅ Parse Claude CLI stop_reason from result messages - COMPLETED 2025-10-08
2. Add turn request counter and MaxTurnRequests handling
3. Verify permission request flow

### Low Priority
1. Add more refusal patterns if needed
2. Improve cancellation timing

## Test Coverage

✅ Extensive testing for:
- Refusal detection (test_is_response_refusal_*)
- Cancellation handling  
- Session lifecycle
- Content validation

✅ Added tests for (2025-10-08):
- MaxTokens stop reason parsing (protocol_translator.rs)
- EndTurn stop reason parsing (protocol_translator.rs)
- Result message parsing (protocol_translator.rs)

❌ Missing tests for:
- MaxTurnRequests stop reason
- Tool call status progression

## Conclusion

**Overall Compliance**: ~90% (Updated 2025-10-08)

**Well Implemented**:
- Core prompt/response flow
- Streaming updates
- Cancellation
- Refusal detection
- Stop reasons (EndTurn, MaxTokens, Cancelled, Refusal)
- Stop reason parsing from Claude CLI

**Needs Work**:
- MaxTurnRequests stop reason (separate issue: acp-implement-max-turn-requests-stop-reason)
- Tool call status reporting verification (separate issue: acp-verify-tool-call-status-updates)

## Implementation History

### 2025-10-08: MaxTokens Stop Reason Implementation
- Added `StreamResult` struct to capture stop_reason (protocol_translator.rs:28-31)
- Implemented `parse_result_message()` method (protocol_translator.rs:206)
- Added `stop_reason` field to `MessageChunk` (claude.rs:54)
- Updated `query_stream()` to parse and propagate stop_reason (claude.rs:243-256)
- Added stop_reason mapping in `handle_prompt()` (agent.rs:1433-1440)
- Comprehensive test coverage for parsing logic (protocol_translator.rs:460-516)
- Build passing, all 711 tests passing, no clippy warnings
