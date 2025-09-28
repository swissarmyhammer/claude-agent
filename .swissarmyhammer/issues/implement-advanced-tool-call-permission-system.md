# Implement Advanced Tool Call Permission System

## Problem
Our permission system doesn't implement the complete `session/request_permission` mechanism with multiple permission options as required by the ACP specification. We need a comprehensive permission system with user choice options and permission persistence.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/tool-calls:

**Permission Request Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "session/request_permission",
  "params": {
    "sessionId": "sess_abc123def456",
    "toolCall": {
      "toolCallId": "call_001"
    },
    "options": [
      {"optionId": "allow-once", "name": "Allow once", "kind": "allow_once"},
      {"optionId": "allow-always", "name": "Allow always", "kind": "allow_always"},
      {"optionId": "reject-once", "name": "Reject", "kind": "reject_once"},
      {"optionId": "reject-always", "name": "Reject always", "kind": "reject_always"}
    ]
  }
}
```

**Permission Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "outcome": {
      "outcome": "selected",
      "optionId": "allow-once"
    }
  }
}
```

## Current Issues
- Permission system exists but may not support full option system
- Missing permission option kinds (allow_once, allow_always, reject_once, reject_always)
- No permission persistence for "always" options
- Missing integration with tool execution flow control

## Implementation Tasks

### Permission Option System
- [ ] Define `PermissionOption` struct with optionId, name, kind
- [ ] Implement `PermissionOptionKind` enum with all ACP-specified types
- [ ] Add permission option generation based on tool and context
- [ ] Support custom permission options for specific tool types
- [ ] Add permission option validation and consistency checking

### Permission Request Implementation
- [ ] Implement `session/request_permission` method handler
- [ ] Generate appropriate permission options for different tool types
- [ ] Include tool call context and details in permission requests
- [ ] Support permission request timeout and cancellation
- [ ] Add permission request correlation with tool calls

### Permission Response Handling
- [ ] Handle all permission outcome types (cancelled, selected)
- [ ] Process selected option IDs and execute corresponding actions
- [ ] Implement permission denial handling with proper error responses
- [ ] Support permission cancellation when prompt turn cancelled
- [ ] Add permission response validation and error handling

### Permission Persistence System
- [ ] Implement permission memory for "always" options
- [ ] Store permission decisions with tool type and context
- [ ] Support permission policy lookup and caching
- [ ] Add permission persistence across agent restarts
- [ ] Implement permission expiration and cleanup

## Permission System Implementation
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub session_id: String,
    pub tool_call: ToolCallUpdate,
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: PermissionOptionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PermissionOutcome {
    Cancelled,
    Selected { option_id: String },
}

impl PermissionSystem {
    pub async fn request_permission(&self, request: PermissionRequest) -> Result<PermissionOutcome>;
    pub fn check_stored_permission(&self, tool_call: &ToolCall) -> Option<PermissionDecision>;
    pub fn store_permission(&self, decision: PermissionDecision);
    pub fn generate_permission_options(&self, tool_call: &ToolCall) -> Vec<PermissionOption>;
}
```

## Implementation Notes
Add advanced permission system comments:
```rust
// ACP requires comprehensive permission system with user choice:
// 1. Multiple permission options: allow/reject with once/always variants
// 2. Permission persistence: Remember "always" decisions across sessions
// 3. Tool call integration: Block execution until permission granted
// 4. Cancellation support: Handle cancelled prompt turns gracefully
// 5. Context awareness: Generate appropriate options for different tools
//
// Advanced permissions provide user control while maintaining security.
```

### Permission Option Generation
- [ ] Generate contextually appropriate permission options
- [ ] Consider tool risk level when providing options
- [ ] Support different option sets for different tool types
- [ ] Add permission option customization based on user preferences
- [ ] Include tool impact assessment in option generation

### Permission Policy Engine
```rust
pub struct PermissionPolicy {
    pub tool_pattern: String,
    pub default_action: PermissionAction,
    pub require_user_consent: bool,
    pub allow_always_option: bool,
}

impl PermissionPolicyEngine {
    pub fn evaluate_tool_call(&self, tool_call: &ToolCall) -> PermissionEvaluation {
        // Check stored permissions first
        if let Some(stored) = self.check_stored_permission(tool_call) {
            return stored.into();
        }
        
        // Evaluate against policies
        for policy in &self.policies {
            if policy.matches(tool_call) {
                return self.apply_policy(policy, tool_call);
            }
        }
        
        // Default to requiring user consent
        PermissionEvaluation::RequireUserConsent
    }
}
```

### Permission Storage and Retrieval
- [ ] Implement persistent permission storage backend
- [ ] Support permission lookup by tool pattern and context
- [ ] Add permission expiration and renewal mechanisms
- [ ] Implement permission storage encryption for security
- [ ] Support permission import/export for user management

### Tool Call Integration
- [ ] Block tool execution until permission granted
- [ ] Handle permission denial with proper tool call failure
- [ ] Integrate permission requests with tool call status updates
- [ ] Support permission request batching for multiple tools
- [ ] Add permission request priority and queuing

### User Experience Enhancements
- [ ] Generate descriptive permission request messages
- [ ] Include tool impact and risk assessment in requests
- [ ] Support permission request localization
- [ ] Add permission request context and help information
- [ ] Implement permission request templates for common scenarios

## Testing Requirements
- [ ] Test complete permission request/response cycle
- [ ] Test all permission option kinds and outcomes
- [ ] Test permission persistence for "always" options
- [ ] Test permission denial and tool call failure handling
- [ ] Test permission cancellation during prompt turn cancellation
- [ ] Test concurrent permission requests and handling
- [ ] Test permission storage and retrieval across restarts
- [ ] Test permission policy evaluation and application

## Configuration and Management
- [ ] Add configurable permission policies and rules
- [ ] Support user-specific permission preferences
- [ ] Add permission audit logging and reporting
- [ ] Implement permission management UI integration
- [ ] Support permission system monitoring and alerting

## Integration Points
- [ ] Connect to existing tool call execution system
- [ ] Integrate with session management and state
- [ ] Connect to user interface for permission displays
- [ ] Integrate with security and audit logging systems

## Acceptance Criteria
- Complete `session/request_permission` implementation with all option types
- Permission persistence for "always" decisions across sessions
- Tool call execution blocking until permission granted
- All permission outcome types handled correctly (cancelled, selected)
- Permission policy engine with customizable rules
- Integration with existing tool call reporting and status systems
- User experience enhancements with descriptive permission requests
- Comprehensive test coverage for all permission scenarios
- Performance optimization for permission checks and storage
- Security measures for permission storage and validation