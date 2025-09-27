# Implement Proper ACP Version Negotiation

## Problem
Our initialization code doesn't implement the version negotiation protocol specified in the ACP specification. The spec requires:
- Client sends latest supported version 
- Agent responds with same version if supported, or its latest version if not
- Client should close connection if agent's version is unsupported

## Current Issues
- We have `ProtocolVersion::V1_0_0` but no negotiation logic
- Version format uses semantic versioning strings instead of spec-required integers
- No handling of unsupported version scenarios

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/initialization:

**Client Request:**
```json
{
  "method": "initialize",
  "params": {
    "protocolVersion": 1,  // Integer, not string
    // ...
  }
}
```

**Agent Response:**
```json
{
  "result": {
    "protocolVersion": 1,  // Same version if supported, or agent's latest
    // ...
  }
}
```

## Implementation Tasks
- [ ] Change `ProtocolVersion` enum to use integer values per spec
- [ ] Implement version negotiation logic in initialization handler
- [ ] Add proper error handling for unsupported versions
- [ ] Update client-side version negotiation to handle agent responses
- [ ] Add validation that both sides agree on protocol version
- [ ] Update tests to cover version negotiation scenarios

## Acceptance Criteria
- Protocol versions use integers (1, 2, etc.) not semantic version strings
- Agent responds with same version if supported, or its latest supported version
- Proper error handling when versions are incompatible
- Client validates agent's version response
- All initialization tests pass with proper version negotiation