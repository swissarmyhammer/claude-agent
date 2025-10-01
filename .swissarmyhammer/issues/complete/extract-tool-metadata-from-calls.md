# Extract Tool Metadata from Tool Calls

## Description
Implement proper extraction of tool name and arguments from tool call objects instead of using placeholder values.

## Locations
- `lib/src/agent.rs:3133` - Tool name extraction
- `lib/src/agent.rs:3134` - Tool arguments extraction

## Code Context
```rust
let tool_name = "unknown_tool"; // TODO: Extract from tool_call
let tool_args = serde_json::json!({}); // TODO: Extract from tool_call
```

## Implementation Notes
- Parse tool_call structure to extract name
- Extract and validate tool arguments
- Handle malformed tool calls gracefully
- Add error handling for missing fields


## Proposed Solution

The agent has access to the `tool_handler` which maintains an `active_tool_calls` HashMap that stores `ToolCallReport` objects keyed by `tool_call_id`. The `ToolCallReport` contains:
- `raw_input: Option<serde_json::Value>` - The tool arguments
- `title: String` - Human-readable title (but not the actual tool name)

However, I discovered that `ToolCallReport` doesn't store the actual tool name, only a `ToolKind` enum category and a human-readable title.

After analysis, I found that:
1. The tool name needs to be extracted from somewhere else in the tool execution flow
2. The `raw_input` field in `ToolCallReport` contains the tool arguments as JSON
3. We need to look up the tool call from `active_tool_calls` using the `tool_call_id` from the request

Implementation approach:
1. Look up the `ToolCallReport` from `self.tool_handler.active_tool_calls` using `request.tool_call.tool_call_id`
2. Extract `tool_args` from `report.raw_input`
3. For the tool name, we need to check if it's stored elsewhere or if we need to modify `ToolCallReport` to include it
4. Add proper error handling for missing tool calls or malformed data

Let me investigate further to find where the actual tool name is available during tool execution.



## Implementation Complete

Successfully implemented proper extraction of tool name and arguments from tool call objects.

### Changes Made

1. **Added `tool_name` field to `ToolCallReport` struct** (lib/src/tool_types.rs:140)
   - Stores the actual tool name for later retrieval
   - Marked with `#[serde(skip)]` to avoid serialization

2. **Updated `ToolCallReport::new()` constructor** (lib/src/tool_types.rs:152)
   - Added `tool_name` parameter
   - Updated all 17 call sites across the codebase

3. **Added `get_active_tool_calls()` method to `ToolCallHandler`** (lib/src/tools.rs:308-311)
   - Returns a snapshot of active tool calls for lookup

4. **Implemented extraction logic in `request_permission`** (lib/src/agent.rs:3132-3151)
   - Looks up tool call from active_tool_calls using tool_call_id
   - Extracts tool_name and raw_input (arguments)
   - Gracefully handles missing tool calls with warning log and defaults

### Error Handling
- When tool_call_id is not found in active_tool_calls, logs a warning and uses default values
- Handles missing raw_input by defaulting to empty JSON object

### Testing
- All 575 existing tests pass
- Changes are backward compatible

The TODO placeholders at agent.rs:3134-3135 have been replaced with proper extraction logic.



## Code Review Completed

All items from the code review have been addressed:

### Lint Errors Fixed
- Deleted 6 unused test helper functions and constants:
  - `resource_link` in content_capability_validator.rs:281
  - `embedded_resource` in content_capability_validator.rs:314
  - `MALICIOUS_ELF_BASE64` in content_security_integration_tests.rs:16
  - `text` in content_security_integration_tests.rs:19
  - `image_png` in content_security_integration_tests.rs:37
  - `resource_link` in content_security_integration_tests.rs:54
- Removed unused `EmbeddedResource` import

### Tests Added
Added 4 comprehensive unit tests for tool metadata extraction in agent.rs:
1. `test_request_permission_extracts_tool_metadata_success` - Verifies successful extraction when tool_call_id exists
2. `test_request_permission_handles_missing_tool_call` - Tests fallback behavior with missing tool_call_id
3. `test_request_permission_with_complex_tool_args` - Tests extraction with complex nested JSON arguments
4. `test_request_permission_with_missing_raw_input` - Tests default to empty object when raw_input is None

### Test Results
- All 579 tests passing
- No compilation warnings or errors
- All clippy checks passing

### Performance Note
The code review identified that `get_active_tool_calls()` clones the entire HashMap. This is noted as an optional future improvement but is acceptable for the current implementation since:
- The method provides necessary abstraction over the internal RwLock
- Tool call volume is typically low
- The optimization can be deferred without blocking merge
