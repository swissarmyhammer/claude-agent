# Implement Plan Notifications via Session Updates

## Problem
Our agent implementation doesn't send plan notifications via `session/update` as required by the ACP specification. Agents should report their execution plans to provide transparency about task planning and progress tracking.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn:

**Plan Update Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "plan",
      "entries": [
        {
          "content": "Check for syntax errors",
          "priority": "high", 
          "status": "pending"
        },
        {
          "content": "Identify potential type issues",
          "priority": "medium",
          "status": "pending"
        },
        {
          "content": "Review error handling patterns",
          "priority": "medium",
          "status": "pending"
        },
        {
          "content": "Suggest improvements",
          "priority": "low",
          "status": "pending"
        }
      ]
    }
  }
}
```

## Current Issues
- No plan generation during prompt processing
- Missing plan reporting via session/update notifications
- No plan entry tracking with status updates
- Missing integration with tool execution and plan completion

## Implementation Tasks

### Plan Data Structures
- [ ] Define `PlanEntry` struct with content, priority, status fields
- [ ] Define `AgentPlan` struct containing list of plan entries
- [ ] Add plan entry status enum (pending, in_progress, completed, failed)
- [ ] Add priority levels enum (high, medium, low)

### Plan Generation Logic
- [ ] Implement plan creation based on user prompt analysis
- [ ] Add heuristics for breaking down complex tasks into steps
- [ ] Generate appropriate priorities for plan entries
- [ ] Create realistic and actionable plan entries

### Plan Update System
- [ ] Send initial plan via session/update notification
- [ ] Send plan updates when entry status changes
- [ ] Report plan progress as entries are completed
- [ ] Handle plan modifications during execution

### Plan Execution Integration
- [ ] Connect plan entries to actual tool executions
- [ ] Update plan entry status when tools start/complete
- [ ] Handle plan entry failures and error scenarios
- [ ] Track overall plan completion progress

## Acceptance Criteria
- Plan generation for all appropriate user requests
- Plans reported via session/update notifications with proper format
- Plan entry status updates sent as work progresses
- Plan integration with tool execution and completion tracking
- Comprehensive test coverage for plan generation and reporting