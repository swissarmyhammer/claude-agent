# Implement Capability-Based Feature Gating

## Problem
Our ACP implementation currently exposes methods regardless of negotiated capabilities. According to the spec, methods should only be available if the corresponding capability was negotiated during initialization.

## Current Issues
- Methods are exposed even if capabilities weren't declared during initialization
- No enforcement of client capability requirements before using features
- Missing proper capability validation throughout the protocol implementation

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/initialization:

**Feature Gating Rules:**
- File system methods only available if client declared `fs` capabilities
- Terminal methods only available if client declared `terminal: true`
- Session loading only available if agent declared `loadSession: true`
- Content types only usable if capabilities were negotiated

## Current Capability Gaps

### Client Capability Validation
Before using client features, verify capabilities were declared:
```rust
// Don't try to write files if client didn't declare writeTextFile: true  
// Don't try to read files if client didn't declare readTextFile: true
// Don't use terminal if client didn't declare terminal: true
```

### Agent Capability Enforcement
Only expose methods if agent declared support:
```rust
// session/load only available if agentCapabilities.loadSession: true
// Prompt content validation based on declared promptCapabilities
```

## Implementation Tasks

### Client Capability Validation
- [ ] Add client capability storage during initialization
- [ ] Validate client has `fs.readTextFile` before attempting file reads
- [ ] Validate client has `fs.writeTextFile` before attempting file writes  
- [ ] Validate client has `terminal: true` before executing terminal commands
- [ ] Return proper errors when client lacks required capabilities

### Agent Method Gating
- [ ] Only register `session/load` handler if `loadSession` capability is true
- [ ] Gate MCP features based on declared MCP capabilities
- [ ] Add capability checking middleware for method routing

### Capability Storage
- [ ] Store negotiated capabilities from initialization
- [ ] Make capabilities accessible throughout request handlers
- [ ] Add capability lookup utilities

### Error Handling
- [ ] Proper error responses when capabilities are missing
- [ ] Clear error messages explaining capability requirements
- [ ] Appropriate error codes per ACP spec

## Error Response Examples
```json
{
  "error": {
    "code": -32601,
    "message": "Method not available: client did not declare terminal capability",
    "data": {
      "method": "terminal/execute",
      "requiredCapability": "terminal",
      "declaredValue": false
    }
  }
}
```

## Implementation Notes
Add clear code comments explaining capability enforcement:
```rust
// ACP requires that we only use features the client declared support for.
// Always check client capabilities before attempting operations.
// This prevents protocol violations and ensures compatibility.
```

## Acceptance Criteria
- All methods gated by appropriate capability checks
- File system operations check client `fs` capabilities
- Terminal operations check client `terminal` capability
- Session loading only available if agent declared support
- Proper error responses for missing capabilities
- Clear error messages explaining requirements
- Comprehensive test coverage for all capability scenarios
- No protocol violations due to capability mismatches