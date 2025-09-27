# Implement Tool Call Permission Request System

## Problem
Our tool execution doesn't implement the `session/request_permission` mechanism as required by the ACP specification. Agents should request permission from clients before executing tools, especially for potentially sensitive operations.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/prompt-turn:

**Permission Request Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/request_permission",
  "params": {
    "sessionId": "sess_abc123def456",
    "toolCallId": "call_001",
    "toolName": "file_analyzer",
    "reason": "Need to analyze the Python file for syntax errors"
  }
}
```

**Expected Client Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "outcome": "granted" | "denied" | "cancelled"
  }
}
```

## Current Issues
- No permission request system before tool execution
- Tools may execute without client consent for sensitive operations
- Missing integration with client capability and security preferences
- No handling of permission denial or cancellation

## Implementation Tasks

### Permission Request Data Structures
- [ ] Define `PermissionRequest` struct with sessionId, toolCallId, toolName, reason
- [ ] Define `PermissionResponse` enum with granted, denied, cancelled outcomes
- [ ] Add proper serialization/deserialization for permission types
- [ ] Define permission request metadata and context

### Permission Request Logic
- [ ] Identify which tools require permission requests
- [ ] Implement permission requirement determination logic
- [ ] Add tool risk assessment for permission decisions
- [ ] Create meaningful permission request reasons
- [ ] Handle different permission levels (read-only vs destructive operations)

### Permission Request Flow
- [ ] Send `session/request_permission` before tool execution
- [ ] Wait for client response before proceeding
- [ ] Handle permission granted scenario (continue execution)
- [ ] Handle permission denied scenario (abort tool call)
- [ ] Handle permission cancelled scenario (cancel entire turn)

### Permission Response Handling
- [ ] Process permission granted responses
- [ ] Handle permission denied with proper error reporting
- [ ] Handle permission cancelled with turn cancellation
- [ ] Add timeout handling for permission requests
- [ ] Support permission response validation

### Tool Execution Integration
- [ ] Integrate permission requests into tool call workflow
- [ ] Block tool execution until permission granted
- [ ] Update tool call status based on permission response
- [ ] Handle permission failures in tool call status updates
- [ ] Coordinate with existing tool call notification system

## Permission Requirements Logic
```rust
#[derive(Debug, Clone)]
pub enum ToolRiskLevel {
    Safe,        // Read-only operations, no permission needed
    Moderate,    // File writes, network requests - may need permission
    Destructive, // File deletion, system changes - always need permission
}

fn requires_permission(tool_name: &str, params: &ToolParams) -> bool {
    match assess_tool_risk(tool_name, params) {
        ToolRiskLevel::Safe => false,
        ToolRiskLevel::Moderate => should_request_permission_for_moderate(),
        ToolRiskLevel::Destructive => true,
    }
}
```

## Implementation Notes
Add permission request comments:
```rust
// ACP allows agents to request permission before tool execution:
// 1. Assess tool risk level and operation type
// 2. Send session/request_permission for sensitive operations  
// 3. Wait for client response before proceeding
// 4. Handle granted/denied/cancelled outcomes appropriately
// 5. Provide clear reasons for permission requests
//
// Permission requests improve security and user control.
```

## Permission Request Reasons
- [ ] Generate clear, specific reasons for permission requests
- [ ] Explain what the tool will do and why it's needed
- [ ] Include context about files, directories, or resources affected
- [ ] Provide user-friendly explanations of tool purposes
- [ ] Support localization of permission request messages

## Error Handling and Edge Cases
- [ ] Handle permission request timeouts
- [ ] Handle client disconnection during permission request
- [ ] Handle malformed permission responses
- [ ] Support permission request retries for transient failures
- [ ] Handle permission denial with graceful degradation

## Security Considerations
- [ ] Implement tool risk assessment logic
- [ ] Support configurable permission policies
- [ ] Add audit logging for permission requests and responses
- [ ] Handle sensitive data in permission request context
- [ ] Support different permission levels for different clients

## Testing Requirements
- [ ] Test permission requests for different tool types
- [ ] Test permission granted flow with successful tool execution
- [ ] Test permission denied flow with proper error handling
- [ ] Test permission cancelled flow with turn cancellation
- [ ] Test permission request timeouts and retries
- [ ] Test integration with existing tool call status system
- [ ] Test permission request reason generation
- [ ] Test concurrent permission requests

## Configuration and Policy
- [ ] Add configurable permission policies per tool type
- [ ] Support client-specific permission preferences
- [ ] Add global permission settings (always grant, always ask, etc.)
- [ ] Support tool-specific permission overrides
- [ ] Add permission request throttling and rate limiting

## Integration Points
- [ ] Connect to existing tool call execution system
- [ ] Integrate with session update notification system
- [ ] Connect to cancellation handling system
- [ ] Integrate with tool call status tracking

## Acceptance Criteria
- Permission request system integrated with tool execution
- `session/request_permission` notifications sent for appropriate tools
- All permission response outcomes handled correctly (granted/denied/cancelled)
- Tool execution blocked until permission granted
- Clear, specific permission request reasons provided
- Integration with existing tool call status and notification system
- Configurable permission policies and risk assessment
- Comprehensive test coverage for all permission scenarios
- Security audit logging for permission requests and responses