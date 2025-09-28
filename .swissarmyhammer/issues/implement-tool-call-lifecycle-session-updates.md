# Implement Tool Call Lifecycle Session Updates

## Problem
Our tool execution doesn't send complete tool call lifecycle updates via `session/update` notifications as required by the ACP specification. We need initial tool call notifications and status updates throughout the tool execution process.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/tool-calls:

**Initial Tool Call Notification:**
```json
{
  "jsonrpc": "2.0",
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

**Progress Update:**
```json
{
  "jsonrpc": "2.0", 
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

**Completion Update:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/update", 
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "tool_call_update",
      "toolCallId": "call_001",
      "status": "completed",
      "content": [
        {
          "type": "content",
          "content": {
            "type": "text",
            "text": "Analysis complete: No syntax errors found..."
          }
        }
      ]
    }
  }
}
```

## Current Issues
- We send tool results but may not send initial tool call notifications
- Missing tool call status updates throughout execution lifecycle
- No tool call progress reporting during execution
- Tool call lifecycle visibility limited for clients

## Implementation Tasks

### Initial Tool Call Notifications
- [ ] Send `tool_call` session update when tool execution begins
- [ ] Include toolCallId, title, kind, and initial pending status
- [ ] Generate appropriate tool titles and descriptions
- [ ] Add tool kind classification for different tool types

### Tool Call Status Updates
- [ ] Send `tool_call_update` when tool execution starts (in_progress)
- [ ] Send progress updates during long-running tool execution
- [ ] Send completion updates with tool results (completed/failed)
- [ ] Support real-time status tracking throughout tool lifecycle

### Tool Call Integration
- [ ] Integrate tool call notifications with existing tool execution
- [ ] Connect tool call updates to tool status changes
- [ ] Add tool call correlation and tracking
- [ ] Support concurrent tool execution with independent notifications

### Tool Result Enhancement
- [ ] Enhance existing tool result reporting with richer content
- [ ] Add tool execution metadata to completion updates
- [ ] Support different content types in tool call results
- [ ] Include tool execution timing and performance data

## Tool Call Lifecycle Implementation
```rust
impl ToolCallNotifier {
    pub async fn notify_tool_call_started(
        &self,
        session_id: &SessionId,
        tool_call_id: &str,
        tool_name: &str,
    ) -> Result<(), NotificationError> {
        let notification = SessionNotification {
            session_id: session_id.clone(),
            update: SessionUpdate::ToolCall(ToolCall {
                tool_call_id: tool_call_id.to_string(),
                title: self.generate_tool_title(tool_name),
                kind: self.classify_tool_kind(tool_name),
                status: ToolCallStatus::Pending,
                content: vec![],
                locations: vec![],
                raw_input: None,
                raw_output: None,
            }),
            meta: None,
        };
        
        self.send_session_update(notification).await
    }
    
    pub async fn notify_tool_call_progress(
        &self,
        session_id: &SessionId,
        tool_call_id: &str,
        status: ToolCallStatus,
    ) -> Result<(), NotificationError> {
        let update = SessionNotification {
            session_id: session_id.clone(),
            update: SessionUpdate::ToolCallUpdate(ToolCallUpdate {
                tool_call_id: tool_call_id.to_string(),
                status: Some(status),
                title: None,
                kind: None,
                content: None,
                locations: None,
                raw_input: None,
                raw_output: None,
            }),
            meta: None,
        };
        
        self.send_session_update(update).await
    }
}
```

## Implementation Notes
Add tool call lifecycle comments:
```rust
// ACP requires complete tool call lifecycle reporting:
// 1. Initial tool_call notification when execution begins
// 2. tool_call_update notifications for status changes
// 3. Progress updates during long-running tool execution
// 4. Final completion updates with results and status
// 5. Tool call correlation and tracking throughout lifecycle
//
// Complete lifecycle reporting provides transparency and enables rich client UX.
```

### Tool Execution Integration
```rust
impl ToolExecutor {
    pub async fn execute_tool_with_notifications(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        params: &ToolParams,
    ) -> Result<ToolResult, ToolError> {
        let tool_call_id = generate_tool_call_id();
        
        // Send initial tool call notification
        self.notifier.notify_tool_call_started(session_id, &tool_call_id, tool_name).await?;
        
        // Update status to in_progress
        self.notifier.notify_tool_call_progress(
            session_id, 
            &tool_call_id,
            ToolCallStatus::InProgress
        ).await?;
        
        // Execute tool
        let result = self.execute_tool_internal(tool_name, params).await;
        
        // Send completion notification
        let final_status = match &result {
            Ok(_) => ToolCallStatus::Completed,
            Err(_) => ToolCallStatus::Failed,
        };
        
        self.notifier.notify_tool_call_completion(
            session_id,
            &tool_call_id,
            final_status,
            &result,
        ).await?;
        
        result
    }
}
```

### Progress Reporting for Long-Running Tools
- [ ] Add progress reporting during file operations
- [ ] Support progress updates for command execution
- [ ] Add percentage completion for quantifiable operations
- [ ] Handle progress reporting for concurrent tool execution

### Tool Call Metadata Enhancement
- [ ] Generate descriptive tool titles based on operation
- [ ] Add tool kind classification (read, edit, execute, etc.)
- [ ] Include tool execution context and parameters
- [ ] Support tool call location tracking

## Testing Requirements
- [ ] Test initial tool call notifications for all tool executions
- [ ] Test tool call status updates throughout execution lifecycle
- [ ] Test tool call completion notifications with results
- [ ] Test concurrent tool execution with independent notifications
- [ ] Test tool call error scenarios and failure notifications
- [ ] Test integration with existing tool result reporting
- [ ] Test tool call correlation and tracking accuracy

## Integration Points
- [ ] Connect to existing tool execution system
- [ ] Integrate with session update notification system
- [ ] Connect to tool registry and classification
- [ ] Integrate with tool result processing and reporting

## Performance Considerations
- [ ] Optimize tool call notification overhead
- [ ] Support efficient tool call tracking and correlation
- [ ] Add tool call notification batching where appropriate
- [ ] Monitor performance impact of additional session updates

## Acceptance Criteria
- Initial tool call notifications sent for all tool executions
- Tool call status updates throughout complete execution lifecycle  
- Tool call completion notifications with results and final status
- Integration with existing tool execution and result systems
- Tool call correlation and tracking across all notifications
- Support for concurrent tool execution with independent tracking
- Performance optimization for tool call notification overhead
- Comprehensive test coverage for all tool call lifecycle scenarios