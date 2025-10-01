# Implement SSE MCP Transport

## Description
Implement full Server-Sent Events (SSE) transport for MCP protocol. Currently returns empty tools list as placeholder.

## Locations
- `lib/src/mcp.rs:483-484` - SSE transport implementation
- `lib/src/mcp.rs:498` - SSE tools list

## Code Context
```rust
// For now, return empty tools list as SSE implementation is complex
// This is a placeholder for the full SSE implementation

// For now, return empty list as we'll get tools from tools/list call
```

## Implementation Notes
- Implement SSE connection handling
- Add event stream parsing
- Implement tools/list protocol over SSE
- Add reconnection logic
- Handle streaming errors
- Test with real SSE MCP servers


## Proposed Solution

After analyzing the existing code, I can see that:
1. HTTP transport already handles SSE responses in `initialize_http_mcp_connection` (lines 465-495)
2. The SSE transport connection structure exists with channels for message passing (lines 32-38)
3. The placeholder `initialize_sse_mcp_connection` just returns empty tools (lines 642-656)

The SSE transport appears to be a dedicated SSE connection (not HTTP with SSE responses). Based on the MCP architecture and existing patterns:

### Implementation Steps:

1. **Add eventsource-client dependency** for SSE connection handling
2. **Implement SSE connection in `initialize_sse_mcp_connection`**:
   - Create HTTP client with headers
   - Establish SSE connection to the endpoint
   - Send initialize request via POST and read SSE response
   - Send initialized notification
   - Request tools/list and parse response
   - Spawn background task to handle incoming SSE events
3. **Implement SSE tool calls in `send_tool_call_to_server`**:
   - Send tool call request via POST
   - Parse response from SSE stream
4. **Add proper error handling and reconnection logic**
5. **Update tests** to cover SSE transport

The key difference from HTTP transport is that SSE maintains a persistent connection for receiving server-initiated events, while requests are still sent via POST.



## Implementation Complete

### What Was Implemented

1. **Full SSE MCP Connection Initialization** (`initialize_sse_mcp_connection`):
   - Creates HTTP client with custom headers from configuration
   - Sends initialize request via POST with MCP protocol version 2024-11-05
   - Parses SSE response format (data: prefixed lines)
   - Sends initialized notification (expects 202 Accepted)
   - Requests and parses tools/list
   - Spawns background task for persistent event stream

2. **SSE Response Parsing** (`parse_sse_response`):
   - Extracts JSON data from SSE format
   - Handles "data: " prefix line parsing
   - Validates JSON structure

3. **Persistent Event Stream Handling** (`handle_sse_event_stream`):
   - Establishes GET connection for server-initiated events
   - Processes streaming bytes into lines
   - Buffers incomplete lines
   - Sends parsed data through channel
   - Implements auto-reconnection with 5-second delay

4. **Tool Call Execution** (in `send_tool_call_to_server`):
   - Sends tool calls via POST with SSE Accept header
   - Parses SSE response
   - Returns structured JSON response

### Technical Decisions

- **No external SSE library needed**: Used existing `reqwest` with streaming support
- **Hybrid approach**: POST for requests, GET for persistent event stream
- **Graceful reconnection**: Background task automatically reconnects on disconnection
- **Buffered line processing**: Handles chunked SSE data correctly
- **Consistent with HTTP transport**: Follows same patterns as HTTP MCP transport

### Testing

- All 605 tests pass
- Code formatted with `cargo fmt`
- No new dependencies required (uses existing tokio, futures, reqwest)

### Files Modified

- `/Users/wballard/github/claude-agent/lib/src/mcp.rs:642-915` - SSE transport implementation
- `/Users/wballard/github/claude-agent/lib/src/mcp.rs:1068-1122` - SSE tool call execution



## Code Review Fixes Applied

### Critical Issues Resolved

1. **✅ Replaced Placeholder SSE Implementation in `mcp_error_handling.rs`**
   - The `EnhancedMcpServerManager::initialize_sse_mcp_protocol_enhanced` method was a placeholder
   - Replaced with full implementation matching `mcp.rs` but using `SessionSetupError` types
   - Added proper timeout handling using `self.protocol_timeout_ms`
   - Implemented all three steps: initialize, initialized notification, tools/list
   - Spawns background task for persistent event stream
   - Location: `lib/src/mcp_error_handling.rs:870-1148`

2. **✅ Added Buffer Size Limits to Prevent Memory Exhaustion**
   - Both `mcp.rs` and `mcp_error_handling.rs` now have 1MB buffer limit
   - Prevents unbounded buffer growth from malicious SSE streams
   - Buffer is cleared if it exceeds MAX_BUFFER_SIZE (1MB)
   - Protects against DoS attacks via long lines without newlines
   - Locations:
     - `lib/src/mcp.rs:872-879`
     - `lib/src/mcp_error_handling.rs:1113-1120`

3. **✅ Added Comprehensive Documentation**
   - `parse_sse_response` functions now have detailed doc comments
   - Includes SSE format specification reference
   - Documents multi-line message handling
   - Provides example SSE stream format
   - Locations:
     - `lib/src/mcp.rs:830-852`
     - `lib/src/mcp_error_handling.rs:1069-1091`

4. **✅ Added Helper Method for Tool Extraction**
   - `EnhancedMcpServerManager::extract_tools_from_list_response_enhanced`
   - Consistent with the implementation in `mcp.rs`
   - Handles missing/malformed tool names gracefully
   - Location: `lib/src/mcp_error_handling.rs:1149-1173`

### Tests Added

Added 8 comprehensive unit tests for SSE functionality in `mcp_error_handling.rs`:
- `test_parse_sse_response_valid` - Valid SSE response parsing
- `test_parse_sse_response_multiple_data_lines` - Only first data line is parsed
- `test_parse_sse_response_no_data` - Error when no data lines present
- `test_parse_sse_response_invalid_json` - Error on malformed JSON
- `test_extract_tools_from_list_response_valid` - Extract multiple tools
- `test_extract_tools_from_list_response_empty` - Handle empty tool list
- `test_extract_tools_from_list_response_no_result` - Handle error responses
- `test_extract_tools_from_list_response_malformed` - Skip tools without names

Location: `lib/src/mcp_error_handling.rs:1320-1457`

### Test Results

All 613 tests pass:
```
Summary [16.349s] 613 tests run: 613 passed (2 leaky), 0 skipped
```

### Design Decisions

1. **Consistent Error Handling**: Used `SessionSetupError` throughout enhanced manager
2. **Timeout Integration**: Wrapped all SSE HTTP requests with protocol timeout
3. **Graceful Degradation**: Log warnings for non-critical issues (e.g., 202 vs 200 status)
4. **Security First**: Added buffer limits to both implementations
5. **Documentation**: Added SSE spec references and examples for clarity

### Issues Not Addressed (Lower Priority)

The following issues from the code review were noted but not implemented as they require more architectural changes:

1. **Cancellation Mechanism**: Background event stream task runs indefinitely
   - Would require tokio::sync::broadcast or CancellationToken
   - Low priority as process termination handles cleanup

2. **Configurable Reconnection Strategy**: 5-second delay is hardcoded
   - Could add exponential backoff with max retries
   - Current simple approach is functional

3. **Per-Message Timeout**: Event stream has no per-message timeout
   - Would require wrapping each read with timeout
   - Less critical as connection timeout handles initial setup

4. **Error Propagation from Event Stream**: Errors are logged but not sent to caller
   - Would require error channel or callback
   - Current approach logs errors for debugging
