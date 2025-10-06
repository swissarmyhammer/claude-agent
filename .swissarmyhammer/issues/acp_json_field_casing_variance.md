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