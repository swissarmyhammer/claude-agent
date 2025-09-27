# Fix ACP Authentication Implementation

## Problem
Our authentication implementation doesn't properly follow the ACP specification for declaring and handling authentication methods. We need to explicitly declare no authentication methods and add clear documentation explaining this architectural decision.

## Current Issues
- `authMethods` array may not be properly populated in initialization response
- Missing clear documentation about why Claude Code doesn't need authentication
- Potential confusion about authentication requirements

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/initialization:

**Required Response:**
```json
{
  "result": {
    "protocolVersion": 1,
    "agentCapabilities": { ... },
    "authMethods": []
  }
}
```

## Architectural Decision
Claude Code is a local development tool that:
- Runs locally on developer machines
- Does not connect to remote services requiring authentication
- Operates within the user's own development environment
- Has no need for user authentication or access control

Therefore, we intentionally provide **no authentication methods**.

## Implementation Tasks
- [ ] Ensure `authMethods: []` is explicitly set in initialization response
- [ ] Add comprehensive code comments explaining the decision
- [ ] Document authentication architecture in relevant modules
- [ ] Add validation that authMethods array is empty
- [ ] Update tests to verify empty authMethods response

## Required Code Comments
Add prominent comments in initialization code:

```rust
// AUTHENTICATION ARCHITECTURE DECISION:
// Claude Code is a local development tool that runs entirely on the user's machine.
// It does not require authentication because:
// 1. It operates within the user's own development environment
// 2. It does not connect to external services requiring credentials
// 3. It has no multi-user access control requirements
// 4. All operations are performed with the user's existing local permissions
//
// Therefore, we intentionally declare NO authentication methods (empty array).
// This is an architectural decision - do not add authentication methods.
// If remote authentication is needed in the future, it should be a separate feature.
```

## Additional Documentation
Add to relevant documentation:
- Architecture decisions document
- API specification comments
- Developer setup instructions

## Implementation Notes
- This is NOT a missing feature - it's an intentional architectural choice
- Future remote features could add authentication if needed
- Local operation model eliminates authentication requirements
- Security is provided by local OS permissions and network isolation

## Acceptance Criteria
- `authMethods` array is explicitly empty in all initialization responses
- Clear code comments explain the architectural decision
- Documentation clarifies why authentication is not needed
- Tests verify empty authMethods response
- No confusion about missing authentication features
- Architecture decision is well documented for future developers