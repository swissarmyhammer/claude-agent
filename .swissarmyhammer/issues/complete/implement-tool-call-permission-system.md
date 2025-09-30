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

## Proposed Solution

After analyzing the codebase, I found that:

1. **Permission infrastructure already exists** in `lib/src/permissions.rs` with:
   - `PermissionPolicyEngine` for evaluating tool calls
   - `StoredPermission` and `PermissionDecision` types
   - `PermissionStorage` trait with file-based implementation
   - Policy evaluation with risk levels (Low, Medium, High, Critical)
   - Default policies for common tool patterns

2. **Tool execution flow** in `lib/src/tools.rs`:
   - `handle_tool_request` creates tool call reports and checks permissions
   - Currently uses simple `requires_permission` check against config lists
   - Has `PermissionRequest` structure but no ACP `session/request_permission` notification

3. **Key integration points**:
   - `ToolCallHandler::handle_tool_request` at line 771
   - `ToolCallHandler::requires_permission` at line 841
   - `NotificationSender` already available for session updates

### Implementation Steps

#### 1. Add ACP Permission Request Types
Create ACP-compliant permission request notification types in `lib/src/tools.rs`:
- `PermissionRequestNotification` with sessionId, toolCallId, toolName, reason
- `PermissionResponse` enum with Granted, Denied, Cancelled outcomes
- Helper methods to generate clear permission request reasons

#### 2. Integrate Permission Policy Engine
Update `ToolCallHandler` to use `PermissionPolicyEngine`:
- Add `permission_engine: PermissionPolicyEngine` field
- Initialize with `FilePermissionStorage` in constructor
- Replace simple `requires_permission` check with policy evaluation

#### 3. Implement Permission Request Flow
Modify `handle_tool_request` to:
- Evaluate tool call with `permission_engine.evaluate_tool_call()`
- On `PolicyEvaluation::RequireUserConsent`, send `session/request_permission` notification
- Wait for client response before proceeding
- Handle Granted/Denied/Cancelled outcomes appropriately
- Store permission decisions for "always" options

#### 4. Add Permission Request Reason Generation
Create `generate_permission_reason` method that:
- Examines tool name and arguments
- Generates specific, user-friendly descriptions
- Examples: "Read configuration file at /etc/app/config.toml", "Execute command: npm install"

#### 5. Handle Permission Responses
Add `handle_permission_response` method that:
- Processes Granted outcome → proceed with tool execution
- Processes Denied outcome → fail tool call with permission denied error
- Processes Cancelled outcome → cancel tool call and potentially entire turn
- Stores "always" decisions via `permission_engine.store_permission_decision()`

#### 6. Testing Strategy
- Test permission requests for different tool risk levels
- Test granted/denied/cancelled flows
- Test "allow always" and "reject always" persistence
- Test permission request timeout handling
- Integration with existing tool call lifecycle tests

### Technical Design

```rust
// New ACP permission request notification
pub struct PermissionRequestParams {
    pub session_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub reason: String,
}

// Modified handle_tool_request flow
pub async fn handle_tool_request(
    &self,
    session_id: &agent_client_protocol::SessionId,
    request: InternalToolRequest,
) -> crate::Result<ToolCallResult> {
    // Create tool call report
    let tool_report = self.create_tool_call_report(...).await;
    
    // Evaluate permission policy
    let evaluation = self.permission_engine
        .evaluate_tool_call(&request.name, &request.arguments)
        .await?;
    
    match evaluation {
        PolicyEvaluation::Allowed => {
            // Execute immediately
            self.execute_tool_with_tracking(session_id, &request, &tool_report).await
        }
        PolicyEvaluation::Denied { reason } => {
            // Fail tool call
            self.fail_tool_call_report(session_id, &tool_report.tool_call_id, ...).await;
            Ok(ToolCallResult::Error(reason))
        }
        PolicyEvaluation::RequireUserConsent { options } => {
            // Send permission request notification
            let reason = self.generate_permission_reason(&request.name, &request.arguments);
            self.send_permission_request(session_id, &tool_report.tool_call_id, &request.name, &reason).await?;
            
            // Return pending status - execution will resume when permission response arrives
            Ok(ToolCallResult::PermissionPending {
                tool_call_id: tool_report.tool_call_id,
                options,
            })
        }
    }
}
```

### Risk Assessment Logic

The existing `permissions.rs` already has risk assessment patterns:
- `fs_read*` → Low risk, auto-allowed
- `fs_write*` → Medium risk, ask user
- `terminal*` → High risk, ask user always
- `http*` → High risk, ask user always

We'll leverage this existing logic and extend it as needed.

### Configuration

Permission policies will be configurable via:
- Default policies in `permissions.rs::default_permission_policies()`
- Session-specific overrides via stored permissions
- Client capabilities influence permission requirements

## Implementation Progress (2025-09-30)

### Phase 1: Permission Engine Integration ✅ COMPLETE

Successfully integrated the `PermissionPolicyEngine` into the tool execution flow:

1. **ToolCallHandler Changes**
   - Added `permission_engine: Arc<PermissionPolicyEngine>` field
   - Updated all constructors to accept permission engine parameter
   - Modified `handle_tool_request` to evaluate permissions before execution

2. **Policy Evaluation Flow**
   - Evaluate tool calls using `permission_engine.evaluate_tool_call()`
   - Handle three policy outcomes:
     - `Allowed` → Execute immediately
     - `Denied` → Fail tool call with reason
     - `RequireUserConsent` → Return PermissionRequired with options

3. **Permission Reason Generation**
   - Implemented `generate_permission_reason()` method
   - Generates user-friendly descriptions from tool name and arguments
   - Examples:
     - fs_read: "Read file at /path/to/file"
     - terminal_create: "Execute command: npm install"
     - fs_write: "Write to file at /path/to/file"

4. **Backward Compatibility**
   - Maintained compatibility with legacy `auto_approved` list
   - Auto-approved tools bypass policy evaluation
   - All 495 existing tests pass without modification

5. **Test Coverage**
   - Updated test helpers to create permission engines
   - All tests passing with permission system active
   - Verified policy evaluation for different risk levels

### Files Modified
- `lib/src/tools.rs` - Core integration
- `lib/src/agent.rs` - Permission engine initialization
- `lib/src/tool_call_lifecycle_tests.rs` - Test helpers

### Remaining Work

**Phase 2: ACP Permission Request Notifications** (NOT STARTED)
- Send `session/request_permission` notification when policy requires consent
- Include sessionId, toolCallId, toolName, and reason in notification
- Wait for client response before proceeding

**Phase 3: Permission Response Handling** (NOT STARTED)
- Implement handler for permission responses
- Process granted/denied/cancelled outcomes
- Store "always" decisions via permission engine
- Resume or fail tool execution based on response

**Phase 4: Integration Tests** (NOT STARTED)
- Test permission request flow for different tool risk levels
- Test granted/denied/cancelled flows
- Test "allow always" and "reject always" persistence
- Test permission request timeout handling

### Technical Notes

The current implementation provides the foundation for the complete permission system:
- Risk-based policy evaluation is working
- Permission reason generation is implemented
- The system correctly identifies when user consent is required

The next phase requires implementing the ACP notification protocol to send permission requests to the client and handle their responses. This will complete the user-facing permission request system.
