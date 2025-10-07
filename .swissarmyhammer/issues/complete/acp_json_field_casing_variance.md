# ACP Spec Variance: JSON Field Naming Convention

## Issue

The server implementation uses snake_case for JSON-RPC field names, but the ACP spec requires camelCase.

## Location

`lib/src/server.rs:381`

## Current Implementation

```rust
let notification_msg = serde_json::json!({
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {
        "session_id": notification.session_id,  // ❌ snake_case
        "update": notification.update,
        "meta": notification.meta
    }
});
```

## Expected Per ACP Spec

According to the [ACP Schema](https://agentclientprotocol.com/protocol/schema#sessionnotification):

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",  // ✅ camelCase
    "update": { ... },
    "_meta": { ... }
  }
}
```

## Fields Affected

The schema shows that all JSON-RPC parameters use camelCase:
- `sessionId` (not `session_id`)
- `protocolVersion` (not `protocol_version`)
- `clientCapabilities` (not `client_capabilities`)
- `agentCapabilities` (not `agent_capabilities`)
- `mcpServers` (not `mcp_servers`)
- `toolCallId` (not `tool_call_id`)
- `toolCall` (not `tool_call`)
- `terminalId` (not `terminal_id`)
- `exitCode` (not `exit_code`)
- `exitStatus` (not `exit_status`)
- `outputByteLimit` (not `output_byte_limit`)
- And many others...

## Impact

- **Compatibility**: Clients implementing the ACP spec correctly will fail to parse messages from this agent
- **Interoperability**: The agent cannot work with ACP-compliant clients/editors  
- **Spec Compliance**: Violates the ACP specification

## Root Cause

Rust's default serde serialization uses snake_case for field names. The `agent_client_protocol` types likely need `#[serde(rename_all = "camelCase")]` attributes to match the spec, but the manual JSON construction in server.rs bypasses this.

## Resolution

1. Verify the `agent_client_protocol` crate types use correct casing with serde attributes
2. Update manual JSON construction in `server.rs` to use camelCase field names
3. Add integration tests that validate JSON-RPC message format against the spec
4. Consider using the protocol types' serialization instead of manual JSON construction

## Testing

Create tests that:
1. Serialize protocol messages and verify field naming
2. Parse example messages from the ACP spec documentation
3. Test round-trip serialization/deserialization with a spec-compliant client

## References

- [ACP Schema Documentation](https://agentclientprotocol.com/protocol/schema)
- [Session Update Notification](https://agentclientprotocol.com/protocol/schema#session%2Fupdate)
- [Initialize Request](https://agentclientprotocol.com/protocol/schema#initializerequest)


## Proposed Solution

After analyzing the codebase, I've identified that:

1. The `agent-client-protocol` crate (v0.4.3) is an external dependency, not part of this workspace
2. The issue is in `lib/src/server.rs:377-385` where manual JSON construction uses snake_case field names
3. The `SessionNotification` struct from the protocol crate likely already has proper serde attributes for camelCase

### Implementation Plan

1. **Write failing tests first** (TDD approach):
   - Create a test that serializes a `SessionNotification` to JSON
   - Verify that the JSON output uses camelCase field names (`sessionId`, not `session_id`)
   - This test will fail with the current manual JSON construction

2. **Fix the issue**:
   - Instead of manually constructing JSON with `serde_json::json!`, serialize the `SessionNotification` struct directly
   - This will use the protocol crate's serde attributes (which should already be correct)
   - If the protocol crate doesn't have proper attributes, we'll need to wrap the notification in a custom struct with correct attributes

3. **Verify the fix**:
   - Run the failing test to confirm it now passes
   - Run all tests to ensure no regressions
   - Check that the JSON-RPC notification format matches the ACP spec

### Key Finding

The problem is that `server.rs` is manually constructing the notification message instead of using the protocol type's serialization:

```rust
let notification_msg = serde_json::json!({
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {
        "session_id": notification.session_id,  // ❌ Manual snake_case
        "update": notification.update,
        "meta": notification.meta
    }
});
```

The fix should serialize the entire notification structure properly, respecting the protocol crate's serde attributes.



## Implementation

### Changes Made

1. **Created test to validate camelCase** (`lib/src/server.rs:642-764`)
   - Test: `test_protocol_type_serialization` - Validates that agent_client_protocol crate uses camelCase
   - Test: `test_session_notification_uses_camel_case` - Validates the server sends camelCase JSON-RPC messages

2. **Created JsonRpcNotification wrapper struct** (`lib/src/server.rs:16-23`)
   - Wraps SessionNotification in proper JSON-RPC format
   - Uses protocol crate's serialization which already implements camelCase

3. **Updated send_notification method** (`lib/src/server.rs:379-402`)
   - Replaced manual JSON construction with structured serialization
   - Uses JsonRpcNotification wrapper to ensure proper format
   - Protocol crate handles camelCase conversion automatically

### Key Findings

1. The `agent-client-protocol` crate (v0.4.3) **already uses camelCase** for serialization
2. The problem was server.rs manually constructing JSON with snake_case field names
3. By using the protocol crate's serialization, we get proper camelCase automatically

### Verification

- Created `test_protocol_type_serialization` to confirm protocol crate uses camelCase
- Created `test_session_notification_uses_camel_case` to validate end-to-end behavior
- All 683 tests pass

### JSON Output Example

Before (manual construction):
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "session_id": "sess_test123",  // ❌ snake_case
    "update": {...},
    "meta": {...}
  }
}
```

After (protocol serialization):
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_test123",  // ✅ camelCase
    "update": {...},
    "_meta": {...}  // ✅ Correct per ACP spec
  }
}
```

### Files Changed

- `lib/src/server.rs`:
  - Added `JsonRpcNotification` struct for proper JSON-RPC wrapping
  - Updated `send_notification` to use structured serialization
  - Added test `test_protocol_type_serialization`
  - Added test `test_session_notification_uses_camel_case`



## Code Review Improvements Completed

### Changes Made

1. **Removed debug output** (lib/src/server.rs:674-677)
   - Removed `eprintln!` statement that was used for debugging during development
   - Test output is now clean without unnecessary debug logging

2. **Added comprehensive documentation** to test functions:
   - `test_protocol_type_serialization`: Documents that it validates the agent_client_protocol crate's camelCase serialization behavior
   - `test_session_notification_uses_camel_case`: Documents that it validates end-to-end JSON-RPC message format with proper field naming

3. **Enhanced JsonRpcNotification documentation** (lib/src/server.rs:16-17)
   - Added detailed explanation of the problem it solves
   - Documented that it replaces manual JSON construction that incorrectly used snake_case
   - Explained how the solution leverages the protocol crate's serde attributes

4. **Converted inline comment to doc comment** on `send_notification` method
   - Moved implementation details from inline comment to proper rustdoc
   - Documented the camelCase serialization behavior
   - Made the method's ACP compliance explicit

### Verification

- All 683 tests pass
- Code quality metrics improved:
  - Documentation complete ✅
  - Debug code removed ✅
  - Follows Rust conventions ✅
  - Type-safe implementation ✅

### Notes

The `terminal_manager.rs` changes noted in the code review are unrelated to this issue. They appear to be from previous commits (removing timeouts, UTF-8 processing). The current branch correctly focuses on the ACP JSON field casing variance fix.
