# Implement Available Commands Updates

## Problem
Our agent implementation doesn't send available commands updates via `session/update` notifications as required by the ACP specification. Clients should be notified when available commands change during session execution.

## ACP Specification Requirements
From agent-client-protocol specification:

**Available Commands Update Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "available_commands_update",
      "availableCommands": [
        {
          "name": "create_plan",
          "description": "Create an execution plan for complex tasks"
        },
        {
          "name": "research_codebase", 
          "description": "Research and analyze the codebase structure"
        }
      ]
    }
  }
}
```

## Current Issues
- No available commands reporting via session updates
- Missing command availability changes during session execution
- No integration with dynamic command registration/deregistration
- Client unaware of command availability changes

## Implementation Tasks

### Available Commands Tracking
- [ ] Track currently available commands per session
- [ ] Detect changes in command availability
- [ ] Send updates when commands become available/unavailable
- [ ] Support dynamic command registration during session

### Command Update Integration
- [ ] Send initial available commands after session creation
- [ ] Update commands when capabilities change
- [ ] Report command changes during tool execution
- [ ] Handle command availability based on session state

## Acceptance Criteria
- Available commands updates sent when command availability changes
- Integration with session management and capability changes
- Dynamic command registration and update reporting
- Comprehensive test coverage for command availability scenarios