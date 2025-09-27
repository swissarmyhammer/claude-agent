# Implement Agent Plan Reporting via session/update

## Problem
Our prompt processing doesn't implement agent plan reporting as required by the ACP specification. Agents should report their execution plans via `session/update` notifications to provide transparency about task planning and progress tracking.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn:

**Agent Plan Update Format:**
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
- No agent plan generation during prompt processing
- Missing plan reporting via `session/update` notifications
- No plan entry tracking with status updates
- Missing integration with tool execution and plan completion

## Implementation Tasks

### Plan Data Structures
- [ ] Define `PlanEntry` struct with content, priority, status fields
- [ ] Define `AgentPlan` struct containing list of plan entries
- [ ] Add plan entry status enum (pending, in_progress, completed, failed)
- [ ] Add priority levels enum (high, medium, low)
- [ ] Implement proper serialization for plan structures

### Plan Generation Logic
- [ ] Implement plan creation based on user prompt analysis
- [ ] Add heuristics for breaking down complex tasks into steps
- [ ] Generate appropriate priorities for plan entries
- [ ] Create realistic and actionable plan entries
- [ ] Handle different types of user requests (code analysis, file operations, etc.)

### Plan Reporting System
- [ ] Send initial plan via `session/update` notification with `plan` type
- [ ] Implement plan update notifications when entry status changes
- [ ] Report plan progress as entries are completed
- [ ] Handle plan modifications during execution
- [ ] Send final plan completion updates

### Plan Execution Integration
- [ ] Connect plan entries to actual tool executions
- [ ] Update plan entry status when tools start (`in_progress`)
- [ ] Update plan entry status when tools complete (`completed`)
- [ ] Handle plan entry failures (`failed` status)
- [ ] Track overall plan completion progress

### Plan Status Management
- [ ] Implement plan entry status transitions
- [ ] Track which entries are currently being executed
- [ ] Handle dependencies between plan entries
- [ ] Support plan entry reordering or modification during execution
- [ ] Add plan completion detection

## Plan Entry Status Lifecycle
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanEntryStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")] 
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
}
```

## Implementation Notes
Add agent plan comments:
```rust
// ACP requires agent plan reporting for transparency and progress tracking:
// 1. Generate actionable plan entries based on user request
// 2. Report initial plan via session/update notification
// 3. Update plan entry status as work progresses
// 4. Connect plan entries to actual tool executions
// 5. Provide clear visibility into agent's approach
//
// Plans should be realistic, specific, and trackable.
```

## Plan Generation Strategies
- [ ] Analyze user prompt to identify required actions
- [ ] Break complex tasks into atomic, executable steps
- [ ] Assign realistic priorities based on task dependencies
- [ ] Create specific, measurable plan entries
- [ ] Consider user's working context and available tools

## Notification Integration
- [ ] Integrate plan updates with existing session update system
- [ ] Ensure plan notifications use proper sessionId
- [ ] Handle plan update timing relative to other notifications
- [ ] Support batched plan updates for efficiency
- [ ] Add plan update queuing and ordering

## Error Handling
- [ ] Handle plan generation failures gracefully
- [ ] Support plan modification when execution encounters issues
- [ ] Add fallback planning when initial approach fails
- [ ] Handle partial plan completion scenarios
- [ ] Provide meaningful error updates in plan entries

## Testing Requirements
- [ ] Test plan generation for various prompt types
- [ ] Test plan reporting via session/update notifications
- [ ] Test plan entry status updates throughout execution
- [ ] Test plan integration with tool execution
- [ ] Test plan modification and error scenarios
- [ ] Test plan completion detection and reporting
- [ ] Test plan notification timing and ordering

## Integration Points
- [ ] Connect to prompt analysis system for plan generation
- [ ] Integrate with tool execution system for status updates
- [ ] Connect to session update notification system
- [ ] Integrate with cancellation system for plan cancellation

## Acceptance Criteria
- Agent plans generated for all appropriate user requests
- Plans reported via `session/update` notifications with proper format
- Plan entries have appropriate content, priority, and initial status
- Plan entry status updates sent as work progresses
- Plan integration with tool execution and completion tracking
- Plan modification support when execution diverges from initial plan
- Comprehensive test coverage for plan generation and reporting
- Clear, actionable plan entries that provide user value