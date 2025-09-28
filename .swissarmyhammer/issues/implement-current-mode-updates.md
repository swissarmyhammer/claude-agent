# Implement Current Mode Updates

## Problem
Our agent implementation doesn't send current mode updates via `session/update` notifications as required by the ACP specification. Clients should be notified when session modes change during execution.

## ACP Specification Requirements
From agent-client-protocol specification:

**Current Mode Update Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456", 
    "update": {
      "sessionUpdate": "current_mode_update",
      "currentModeId": "planning_mode"
    }
  }
}
```

## Current Issues
- No session mode tracking or reporting
- Missing current mode updates during mode transitions
- No integration with session mode management
- Client unaware of session mode changes

## Implementation Tasks

### Session Mode Management
- [ ] Implement session mode tracking and transitions
- [ ] Add mode change detection and notification
- [ ] Support different session modes (normal, planning, execution, etc.)
- [ ] Handle mode transitions during session execution

### Mode Update Integration
- [ ] Send mode updates when session mode changes
- [ ] Report initial mode after session creation
- [ ] Update mode during different phases of execution
- [ ] Handle mode-specific capabilities and behavior

## Acceptance Criteria
- Session mode tracking and transition management
- Current mode updates sent when modes change
- Integration with session management and execution phases
- Comprehensive test coverage for mode change scenarios