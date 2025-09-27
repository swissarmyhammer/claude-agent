# Implement Comprehensive Tool Call Status Reporting

## Problem
Our tool call execution doesn't implement the complete status lifecycle reporting required by the ACP specification. We need comprehensive tool call status updates throughout the entire execution process from initial call to completion.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn:

**Tool Call Status Lifecycle:**

**1. Initial Tool Call:**
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456", 
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "call_001",
      "title": "Analyzing Python code",
      "kind": "other",
      "status": "pending"
    }
  }
}
```

**2. Progress Update:**
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "tool_call_update", 
      "toolCallId": "call_001",
      "status": "in_progress"
    }
  }
}
```

**3. Completion Update:**
```json
{
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "call_001", 
      "status": "completed",
      "content": [
        {
          "type": "text",
          "text": "Analysis complete: No syntax errors found..."
        }
      ]
    }
  }
}
```

## Current Issues
- Tool call status tracking may be incomplete
- Missing comprehensive status lifecycle (pending → in_progress → completed/failed)
- No real-time progress updates during tool execution
- Missing proper content reporting in completion updates
- No error status reporting for failed tools

## Implementation Tasks

### Tool Call Status Types
- [ ] Define complete `ToolCallStatus` enum with all required variants
- [ ] Add `pending`, `in_progress`, `completed`, `failed`, `cancelled` statuses
- [ ] Implement proper serialization for tool call statuses
- [ ] Add status validation and transition logic

### Initial Tool Call Reporting
- [ ] Send initial `tool_call` notification when tool is requested
- [ ] Include toolCallId, title, kind, and initial `pending` status
- [ ] Generate appropriate tool titles and descriptions
- [ ] Implement tool kind classification (other, file_operation, etc.)

### Progress Status Updates
- [ ] Send `tool_call_update` when tool execution starts (`in_progress`)
- [ ] Support intermediate progress updates during long-running tools
- [ ] Add progress percentage or descriptive progress messages
- [ ] Handle concurrent tool execution status tracking

### Completion Status Updates
- [ ] Send `tool_call_update` when tool completes successfully (`completed`)
- [ ] Include tool execution results in content field
- [ ] Format tool results appropriately for client consumption
- [ ] Handle different types of tool output (text, files, structured data)

### Error Status Updates
- [ ] Send `tool_call_update` when tool fails (`failed`)
- [ ] Include error information and diagnostic details
- [ ] Provide actionable error messages for users
- [ ] Handle different types of tool failures gracefully

### Cancellation Status Updates
- [ ] Send `tool_call_update` when tool is cancelled (`cancelled`)
- [ ] Handle client-initiated tool cancellation
- [ ] Update status immediately when cancellation requested
- [ ] Clean up tool resources on cancellation

## Tool Call Status Lifecycle Implementation
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCallStatus {
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

pub struct ToolCallTracker {
    call_id: String,
    status: ToolCallStatus,
    start_time: SystemTime,
    progress_updates: Vec<String>,
}
```

## Implementation Notes
Add comprehensive tool call status comments:
```rust
// ACP requires complete tool call status lifecycle reporting:
// 1. Initial tool_call notification with pending status
// 2. tool_call_update to in_progress when execution starts
// 3. Optional progress updates during long-running operations
// 4. Final tool_call_update with completed/failed/cancelled status
// 5. Include results/errors in final update content
//
// Status updates provide transparency and enable client UI updates.
```

### Real-time Progress Updates
- [ ] Support streaming progress updates for long-running tools
- [ ] Add progress percentage tracking where applicable
- [ ] Include descriptive progress messages (e.g., "Processing file 3 of 10")
- [ ] Handle progress updates without overwhelming the client
- [ ] Support cancellation of in-progress operations

### Content Formatting
- [ ] Format tool results appropriately for different content types
- [ ] Handle large tool outputs with pagination or truncation
- [ ] Support structured data results (JSON, tables, etc.)
- [ ] Include metadata about tool execution (duration, resource usage)
- [ ] Handle binary or non-text tool outputs

### Tool Call ID Management
- [ ] Generate unique tool call IDs for tracking
- [ ] Maintain tool call registry for status lookups
- [ ] Handle tool call ID conflicts and validation
- [ ] Support tool call correlation across notifications

### Error Handling and Diagnostics
- [ ] Capture detailed error information from failed tools
- [ ] Provide actionable error messages and suggestions
- [ ] Include diagnostic context for debugging
- [ ] Handle tool timeout errors specifically
- [ ] Support error recovery and retry scenarios

## Testing Requirements
- [ ] Test complete tool call status lifecycle for successful tools
- [ ] Test tool call failure scenarios with proper error status
- [ ] Test tool call cancellation with immediate status updates
- [ ] Test concurrent tool execution with independent status tracking
- [ ] Test progress updates during long-running tool execution
- [ ] Test different types of tool output content formatting
- [ ] Test tool call ID uniqueness and tracking
- [ ] Test status transition validation and error handling

## Integration Points
- [ ] Connect to existing tool execution system
- [ ] Integrate with session update notification system
- [ ] Connect to cancellation handling for cancelled status
- [ ] Integrate with permission system for blocked tools

## Performance Considerations
- [ ] Optimize status update frequency for long-running tools
- [ ] Handle large tool outputs efficiently
- [ ] Support batching of status updates where appropriate
- [ ] Add tool execution timeout and resource limits

## Acceptance Criteria
- Complete tool call status lifecycle implemented (pending → in_progress → completed/failed/cancelled)
- Initial `tool_call` notifications sent with proper metadata
- Progress `tool_call_update` notifications during execution
- Final `tool_call_update` notifications with results/errors
- Real-time progress updates for long-running operations
- Proper content formatting for different tool output types
- Tool call cancellation with immediate status updates
- Comprehensive test coverage for all status scenarios
- Integration with existing tool and notification systems