# Implement Terminal Embedding in Tool Calls

## Problem
Our terminal implementation may not support embedding terminals in tool calls as required by the ACP specification. This feature allows clients to display live terminal output within tool call progress updates.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/terminals:

**Terminal Embedding in Tool Calls:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "sess_abc123def456",
    "update": {
      "sessionUpdate": "tool_call",
      "toolCallId": "call_002",
      "title": "Running tests",
      "kind": "execute", 
      "status": "in_progress",
      "content": [
        {
          "type": "terminal",
          "terminalId": "term_xyz789"
        }
      ]
    }
  }
}
```

**Key Behaviors:**
- Terminals can be embedded directly in tool call content
- Client displays live output as it's generated
- Client continues to display output even after terminal is released
- Integration with tool call lifecycle and status updates

## Current Issues
- Terminal embedding in tool calls unclear
- No integration between terminal system and tool call content
- Missing live output display coordination with clients
- Tool call terminal content type may not be implemented

## Implementation Tasks

### Terminal Content Type Implementation
- [ ] Implement `terminal` content type for tool calls
- [ ] Add terminal content serialization with terminalId field
- [ ] Support terminal content validation and consistency checking
- [ ] Integrate with existing tool call content system

### Tool Call Integration
- [ ] Add terminal embedding support to tool call reporting
- [ ] Connect terminal creation with tool call status updates
- [ ] Support terminal content updates during tool execution
- [ ] Handle terminal lifecycle within tool call context

### Live Output Coordination
- [ ] Coordinate terminal output streaming with tool call updates
- [ ] Support real-time output visibility through tool call content
- [ ] Add output update notifications during terminal execution
- [ ] Handle output persistence after terminal release

### Terminal-Tool Call Lifecycle
- [ ] Create terminals within tool call execution context
- [ ] Embed terminal content in initial tool call notification
- [ ] Update tool call status based on terminal execution progress
- [ ] Handle terminal completion and tool call status coordination

## Terminal Embedding Implementation
```rust
use crate::tool_calls::{ToolCallContent, ToolCall};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalContent {
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

impl ToolCallContent {
    pub fn terminal(terminal_id: String) -> Self {
        Self::Terminal {
            terminal_id,
        }
    }
}

impl ToolExecutor {
    pub async fn execute_command_with_terminal_embedding(
        &self,
        tool_call_id: String,
        command: &str,
        args: &[String],
    ) -> Result<ToolResult, ToolError> {
        // Create terminal
        let terminal_id = self.create_terminal(command, args).await?;
        
        // Send initial tool call with embedded terminal
        self.send_tool_call_update(ToolCallUpdate {
            tool_call_id: tool_call_id.clone(),
            status: Some(ToolCallStatus::InProgress),
            content: Some(vec![ToolCallContent::terminal(terminal_id.clone())]),
            ..Default::default()
        }).await?;
        
        // Wait for terminal completion
        let exit_status = self.wait_for_terminal_exit(&terminal_id).await?;
        
        // Update tool call with completion status
        let final_status = if exit_status.exit_code == Some(0) {
            ToolCallStatus::Completed
        } else {
            ToolCallStatus::Failed
        };
        
        self.send_tool_call_update(ToolCallUpdate {
            tool_call_id,
            status: Some(final_status),
            ..Default::default()
        }).await?;
        
        // Note: Don't release terminal here - client should continue displaying output
        
        Ok(ToolResult::success())
    }
}
```

## Implementation Notes
Add terminal embedding comments:
```rust
// ACP terminal embedding in tool calls enables rich user experience:
// 1. Create terminal and embed in tool call content immediately
// 2. Client displays live output as terminal runs
// 3. Tool call status updates based on terminal execution
// 4. Terminal output persists after release for continued display
// 5. Integration with tool call lifecycle and progress reporting
//
// Embedded terminals provide transparency and real-time feedback.
```

### Output Persistence After Release
- [ ] Ensure terminal output remains accessible to client after release
- [ ] Handle output display continuation in client UI
- [ ] Support output persistence across session boundaries
- [ ] Add output archival for completed terminals

### Tool Call Status Coordination
```rust
impl TerminalToolCallCoordinator {
    pub async fn coordinate_terminal_with_tool_call(
        &self,
        tool_call_id: String,
        terminal_id: String,
    ) -> Result<(), CoordinationError> {
        // Monitor terminal status and update tool call accordingly
        let terminal = self.get_terminal(&terminal_id)?;
        
        loop {
            match terminal.get_status().await? {
                TerminalStatus::Running => {
                    // Keep tool call in progress
                    continue;
                }
                TerminalStatus::Completed(exit_status) => {
                    let tool_status = if exit_status.exit_code == Some(0) {
                        ToolCallStatus::Completed
                    } else {
                        ToolCallStatus::Failed
                    };
                    
                    self.update_tool_call_status(tool_call_id, tool_status).await?;
                    break;
                }
                TerminalStatus::Failed(error) => {
                    self.update_tool_call_status(tool_call_id, ToolCallStatus::Failed).await?;
                    break;
                }
            }
            
            // Check periodically
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Ok(())
    }
}
```

### Client Display Integration
- [ ] Support client terminal display within tool call UI
- [ ] Add terminal output formatting and presentation
- [ ] Handle terminal display persistence after tool completion
- [ ] Support terminal output scrolling and interaction

### Terminal Content Validation
- [ ] Validate terminal IDs in tool call content
- [ ] Ensure terminals exist before embedding in tool calls
- [ ] Add terminal content consistency checking
- [ ] Handle terminal content errors gracefully

### Multiple Terminal Support
- [ ] Support multiple terminals per tool call
- [ ] Handle concurrent terminal execution in single tool
- [ ] Add terminal coordination and synchronization
- [ ] Support terminal dependency management

## Testing Requirements
- [ ] Test terminal embedding in tool call content
- [ ] Test live output display coordination with clients
- [ ] Test terminal output persistence after release
- [ ] Test tool call status updates based on terminal execution
- [ ] Test multiple terminals per tool call
- [ ] Test terminal embedding error scenarios
- [ ] Test client display integration and formatting

## Integration Points
- [ ] Connect to tool call content and reporting system
- [ ] Integrate with terminal lifecycle management
- [ ] Connect to session update notification system
- [ ] Integrate with client display and UI coordination

## Client Coordination
- [ ] Design client-agent protocol for terminal display coordination
- [ ] Add terminal output streaming to embedded terminals
- [ ] Support terminal interaction and input from clients
- [ ] Handle terminal display preferences and formatting

## Acceptance Criteria
- Terminal embedding in tool call content with `terminal` content type
- Live output coordination between terminals and tool call displays
- Terminal output persistence in client UI after release
- Tool call status updates based on terminal execution results
- Support for multiple terminals per tool call
- Integration with existing tool call and terminal systems
- Client display coordination and formatting support
- Comprehensive test coverage for all embedding scenarios