# Implement Client Terminal Capability Validation

## Problem
Our terminal operations don't properly validate client capabilities before attempting terminal operations as required by the ACP specification. We need to check that the client has declared support for terminal operations before calling any terminal methods.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/terminals:

**Required Capability Check:**
> "Before attempting to use terminal methods, Agents MUST verify that the Client supports this capability by checking the Client Capabilities field in the initialize response"

**Capability Structure:**
```json
{
  "clientCapabilities": {
    "terminal": true
  }
}
```

**Compliance Rule:**
> "If terminal is false or not present, the Agent MUST NOT attempt to call any terminal methods."

## Current Issues
- Terminal operations may be attempted without checking client capabilities
- No validation that client supports terminal operations
- Missing proper error handling for unsupported terminal capability
- Terminal method availability may not be gated by capability

## Implementation Tasks

### Capability Validation Infrastructure
- [ ] Store client terminal capability from initialization response
- [ ] Make terminal capability accessible throughout agent lifecycle
- [ ] Add capability lookup utilities for terminal validation
- [ ] Ensure capability persists across session operations

### Terminal Method Capability Guards
- [ ] Check `clientCapabilities.terminal` before any terminal operations
- [ ] Reject terminal requests if capability not declared
- [ ] Return proper ACP error for unsupported terminal operations
- [ ] Add capability checking middleware for all terminal methods

### Method Registration with Capability Guards
- [ ] Only register terminal methods if client supports terminal capability
- [ ] Add capability-based method routing for terminal operations
- [ ] Implement method availability checking
- [ ] Handle method registration dynamically based on capabilities

### Tool Integration Capability Checking
- [ ] Check terminal capability before exposing terminal-related tools
- [ ] Validate terminal capability for tool call embedding
- [ ] Return capability-appropriate tool lists
- [ ] Handle tool execution failures due to capability limitations

## Capability Validation Implementation
```rust
pub struct TerminalCapabilityValidator {
    terminal_supported: bool,
}

impl TerminalCapabilityValidator {
    pub fn new(client_capabilities: &ClientCapabilities) -> Self {
        Self {
            terminal_supported: client_capabilities.terminal.unwrap_or(false),
        }
    }
    
    pub fn validate_terminal_operation(&self) -> Result<(), CapabilityError> {
        if !self.terminal_supported {
            return Err(CapabilityError::TerminalNotSupported);
        }
        Ok(())
    }
    
    pub fn is_terminal_supported(&self) -> bool {
        self.terminal_supported
    }
}

impl TerminalMethodHandler {
    pub fn register_methods(
        &self,
        router: &mut MethodRouter,
        capabilities: &TerminalCapabilityValidator,
    ) {
        if capabilities.is_terminal_supported() {
            router.register("terminal/create", self.handle_terminal_create());
            router.register("terminal/output", self.handle_terminal_output());
            router.register("terminal/wait_for_exit", self.handle_terminal_wait_for_exit());
            router.register("terminal/kill", self.handle_terminal_kill());
            router.register("terminal/release", self.handle_terminal_release());
        }
    }
}
```

## Implementation Notes
Add terminal capability validation comments:
```rust
// ACP requires strict terminal capability validation:
// 1. MUST check clientCapabilities.terminal before any terminal operations
// 2. MUST NOT attempt terminal methods if capability not declared
// 3. MUST return proper errors for unsupported operations
// 4. Only register terminal methods if client supports capability
//
// This prevents protocol violations and ensures client compatibility.
```

### Error Response Implementation
For unsupported terminal operations:
```json
{
  "error": {
    "code": -32601,
    "message": "Method not supported: client does not support terminal operations",
    "data": {
      "method": "terminal/create",
      "requiredCapability": "clientCapabilities.terminal",
      "declaredValue": false,
      "supportedMethods": ["fs/read_text_file", "fs/write_text_file"]
    }
  }
}
```

### Capability-Based Tool Availability
```rust
impl ToolRegistry {
    pub fn get_available_tools(&self, capabilities: &ClientCapabilities) -> Vec<ToolDefinition> {
        let mut tools = self.get_base_tools();
        
        if capabilities.terminal.unwrap_or(false) {
            tools.extend(self.get_terminal_tools());
        }
        
        if let Some(fs_caps) = &capabilities.fs {
            if fs_caps.read_text_file {
                tools.extend(self.get_file_read_tools());
            }
            if fs_caps.write_text_file {
                tools.extend(self.get_file_write_tools());
            }
        }
        
        tools
    }
}
```

### Integration with Existing Systems
- [ ] Connect to session management for capability storage
- [ ] Integrate with tool registration and availability
- [ ] Connect to error handling and response systems
- [ ] Add capability validation to existing terminal code

## Testing Requirements
- [ ] Test terminal operations rejected when `terminal: false`
- [ ] Test terminal operations rejected when terminal capability not declared
- [ ] Test proper error responses for unsupported terminal capability
- [ ] Test method registration based on declared terminal capability
- [ ] Test tool availability based on terminal capability
- [ ] Test capability validation integration with existing terminal operations

## Integration Points
- [ ] Connect to initialization capability storage
- [ ] Integrate with terminal method handlers
- [ ] Connect to tool registration and availability system
- [ ] Integrate with error handling and response systems

## Performance Considerations
- [ ] Cache capability lookups to avoid repeated validation
- [ ] Optimize capability checking overhead in terminal operations
- [ ] Support efficient capability-based method routing
- [ ] Minimize validation impact on terminal operation performance

## Acceptance Criteria
- Terminal operations only attempted when client capability allows
- All terminal methods only called when `clientCapabilities.terminal: true`
- Proper ACP error responses for unsupported terminal operations
- Method registration based on declared client terminal capability
- Integration with existing terminal and tool systems
- Comprehensive test coverage for all capability scenarios
- Performance optimization for capability validation overhead
- Documentation of terminal capability requirements and error handling