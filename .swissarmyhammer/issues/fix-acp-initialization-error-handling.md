# Fix ACP Initialization Error Handling

## Problem
Our initialization implementation lacks proper error handling for version negotiation failures and capability mismatches as required by the ACP specification. We need comprehensive error responses and connection management.

## Current Issues
- Missing proper version negotiation failure handling
- No error responses for capability validation failures
- Inadequate connection management for incompatible clients
- Missing structured error responses per ACP spec

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/initialization:

**Version Negotiation Failures:**
- Agent should respond with its latest supported version if client version unsupported
- Client should close connection if agent version is unsupported
- Proper error communication for version mismatches

**Capability Validation:**
- Validate all capability structures during initialization
- Reject malformed capability declarations
- Provide clear error messages for invalid capabilities

## Error Scenarios to Handle

### Version Negotiation Errors
```json
{
  "error": {
    "code": -32600,
    "message": "Protocol version 2 not supported. Latest supported version is 1.",
    "data": {
      "requestedVersion": 2,
      "supportedVersion": 1,
      "action": "downgrade_or_disconnect"
    }
  }
}
```

### Capability Validation Errors
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid client capabilities: unknown capability 'customExtension'",
    "data": {
      "invalidCapability": "customExtension",
      "supportedCapabilities": ["fs", "terminal"]
    }
  }
}
```

### Malformed Request Errors
```json
{
  "error": {
    "code": -32600,
    "message": "Invalid initialize request: missing required field 'protocolVersion'",
    "data": {
      "missingFields": ["protocolVersion"],
      "receivedParams": "..."
    }
  }
}
```

## Implementation Tasks

### Version Negotiation Error Handling
- [ ] Add version compatibility checking logic
- [ ] Implement proper version negotiation responses
- [ ] Add connection termination for incompatible versions
- [ ] Create version mismatch error responses

### Capability Validation
- [ ] Add comprehensive capability structure validation
- [ ] Validate all client capability declarations
- [ ] Check for unknown or unsupported capabilities
- [ ] Validate capability value types and constraints

### Error Response Implementation  
- [ ] Create structured error response types
- [ ] Implement proper ACP error codes
- [ ] Add detailed error data for debugging
- [ ] Ensure error messages are client-helpful

### Connection Management
- [ ] Add graceful connection termination for fatal errors
- [ ] Implement proper cleanup for failed initialization
- [ ] Add logging for initialization failures
- [ ] Handle partial initialization states

## Error Code Standards
Use appropriate JSON-RPC error codes:
- `-32600`: Invalid Request (malformed initialization)
- `-32602`: Invalid params (bad capabilities) 
- `-32603`: Internal error (server-side failures)
- Custom codes for protocol-specific errors

## Implementation Notes
Add comprehensive error handling:
```rust
// ACP requires robust error handling during initialization.
// All error responses must include:
// 1. Appropriate JSON-RPC error codes
// 2. Clear, actionable error messages  
// 3. Structured data for programmatic handling
// 4. Proper connection management
```

## Testing Requirements
- [ ] Test all version negotiation failure scenarios
- [ ] Test capability validation with invalid inputs
- [ ] Test malformed initialization requests
- [ ] Test connection termination for fatal errors
- [ ] Test error response structure compliance
- [ ] Test graceful degradation for partial capability support

## Acceptance Criteria
- Comprehensive error handling for all initialization failure modes
- Proper version negotiation error responses
- Robust capability validation with clear error messages
- Structured error responses following JSON-RPC standards
- Graceful connection management for incompatible clients
- Clear error data for client debugging and recovery
- Complete test coverage for error scenarios
- Proper logging for initialization failures