# Add MCP Server Validation

## Description
Add proper MCP server validation once types are aligned. Currently, validation is skipped during session loading due to type mismatches.

## Location
`lib/src/session_loading.rs:316`

## Code Context
```rust
// TODO: Add proper MCP server validation once types are aligned
// For now, we'll skip MCP server validation as the types don't match
```

## Implementation Notes
- Need to align MCP server types across the codebase
- Implement proper validation logic for MCP servers during session loading
- Ensure validation covers all required fields and constraints


## Proposed Solution

After analyzing the codebase, I've identified the following approach:

### Current State
- `LoadSessionRequest` contains `mcp_servers: Vec<agent_client_protocol::McpServer>`
- The TODO at `session_loading.rs:316` indicates validation is skipped due to type mismatches
- Validation functions already exist in `session_validation.rs` for internal `McpServerConfig` types

### Solution Steps

1. **Create a conversion helper**: Similar to the existing `convert_acp_to_internal_mcp_config` method in agent.rs, create a function to convert ACP MCP servers to internal types for validation purposes.

2. **Implement validation in session_loading.rs**: 
   - Convert each `agent_client_protocol::McpServer` to `crate::config::McpServerConfig`
   - Call existing `validate_mcp_server_config` function for each server
   - Return appropriate errors if validation fails

3. **Add tests**: Create tests to verify MCP server validation works correctly for:
   - Valid Stdio servers
   - Valid HTTP servers  
   - Valid SSE servers
   - Invalid configurations (bad URLs, missing executables, etc.)

### Implementation Details
- Reuse existing validation logic in `session_validation::validate_mcp_server_config`
- Follow the pattern used in `new_session` validation (lines 2152-2157 in agent.rs)
- Ensure error messages are clear and actionable



## Implementation Notes

### Changes Made
1. **Added conversion helper method** (`convert_acp_to_internal_mcp_config`):
   - Converts `agent_client_protocol::McpServer` to `crate::config::McpServerConfig`
   - Handles all three transport types: Stdio, Http, and Sse
   - Mirrors the implementation in `agent.rs` for consistency

2. **Updated `validate_load_request` method**:
   - Removed TODO comment
   - Added loop to validate each MCP server in the request
   - Converts ACP format to internal format for validation
   - Reuses existing validation logic from `session_validation::validate_mcp_server_config`
   - Logs warning if conversion fails (though this should never happen)

### Design Decisions
- Made the conversion helper a static method (doesn't need `&self`)
- Returns `Option<McpServerConfig>` to handle potential conversion failures gracefully
- Reused existing validation infrastructure rather than duplicating logic
- Maintained consistency with the validation pattern used in `agent.rs`



## Test Results

### Tests Added
1. `test_validate_load_request_with_valid_stdio_server` - Validates Stdio server with relative command path
2. `test_validate_load_request_with_valid_http_server` - Validates HTTP server with proper URL
3. `test_validate_load_request_with_valid_sse_server` - Validates SSE server with proper URL
4. `test_validate_load_request_with_invalid_http_url` - Ensures invalid HTTP URLs are rejected
5. `test_validate_load_request_with_invalid_sse_url` - Ensures invalid SSE URLs are rejected
6. `test_validate_load_request_with_multiple_servers` - Tests validation with all three transport types
7. `test_validate_load_request_with_nonexistent_stdio_command` - Ensures nonexistent absolute command paths are rejected

### Build and Test Results
- ✅ `cargo build` succeeded without errors
- ✅ `cargo nextest run` passed all 506 tests (including 7 new tests)
- ✅ No compilation warnings
- ✅ All existing tests continue to pass

## Summary

The MCP server validation has been successfully implemented in the session loading module. The solution:

1. Converts ACP protocol MCP server configs to internal types for validation
2. Reuses existing validation logic from `session_validation.rs`
3. Provides comprehensive error messages for validation failures
4. Maintains consistency with the validation pattern used elsewhere in the codebase
5. Is fully tested with both positive and negative test cases

The TODO at `session_loading.rs:316` has been resolved, and MCP servers are now properly validated during session loading.
