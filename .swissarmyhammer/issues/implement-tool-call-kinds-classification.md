# Implement Tool Call Kinds (ToolKind) Classification

## Problem
Our tool call reporting doesn't implement the tool kind classification system required by the ACP specification. Tool kinds help clients choose appropriate icons and optimize how they display tool execution progress.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/tool-calls:

**Required Tool Kinds:**
- `read` - Reading files or data
- `edit` - Modifying files or content
- `delete` - Removing files or data
- `move` - Moving or renaming files
- `search` - Searching for information
- `execute` - Running commands or code
- `think` - Internal reasoning or planning
- `fetch` - Retrieving external data
- `other` - Other tool types (default)

**Tool Kind Usage:**
```json
{
  "sessionUpdate": "tool_call",
  "toolCallId": "call_001",
  "title": "Reading configuration file",
  "kind": "read",
  "status": "pending"
}
```

## Current Issues
- Tool kind classification and reporting unclear
- No automatic tool kind detection based on tool operation
- Missing tool kind validation and consistency
- No integration with client UI optimization hints

## Implementation Tasks

### Tool Kind Enumeration
- [ ] Define complete `ToolKind` enum with all ACP-specified variants
- [ ] Add proper serialization/deserialization for tool kinds
- [ ] Implement tool kind validation and consistency checking
- [ ] Support tool kind defaulting to `other` when unspecified

### Tool Kind Classification Logic
- [ ] Implement automatic tool kind detection based on tool name/operation
- [ ] Add tool kind assignment rules for common tool patterns
- [ ] Support manual tool kind override for specific tools
- [ ] Create tool kind mapping configuration system
- [ ] Add tool kind inference from tool parameters and context

### Tool Kind Integration
- [ ] Integrate tool kind assignment into tool call reporting
- [ ] Add tool kind validation during tool execution
- [ ] Support tool kind updates during tool lifecycle
- [ ] Include tool kind in tool call status updates
- [ ] Add tool kind filtering and querying capabilities

## Tool Kind Implementation
```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Edit, 
    Delete,
    Move,
    Search,
    Execute,
    Think,
    Fetch,
    Other,
}

impl ToolKind {
    pub fn from_tool_name(tool_name: &str) -> Self {
        match tool_name {
            "read_file" | "cat" | "head" | "tail" => ToolKind::Read,
            "write_file" | "edit_file" | "modify" => ToolKind::Edit,
            "delete_file" | "rm" | "remove" => ToolKind::Delete,
            "move_file" | "mv" | "rename" => ToolKind::Move,
            "grep" | "find" | "search" => ToolKind::Search,
            "bash" | "shell" | "execute" | "run" => ToolKind::Execute,
            "think" | "reason" | "plan" => ToolKind::Think,
            "curl" | "wget" | "fetch" | "download" => ToolKind::Fetch,
            _ => ToolKind::Other,
        }
    }
    
    pub fn from_operation(operation: &ToolOperation) -> Self {
        match operation {
            ToolOperation::FileRead(_) => ToolKind::Read,
            ToolOperation::FileWrite(_) => ToolKind::Edit,
            ToolOperation::FileDelete(_) => ToolKind::Delete,
            ToolOperation::FileMove(_, _) => ToolKind::Move,
            ToolOperation::Search(_) => ToolKind::Search,
            ToolOperation::CommandExecution(_) => ToolKind::Execute,
            ToolOperation::HttpRequest(_) => ToolKind::Fetch,
            ToolOperation::InternalReasoning => ToolKind::Think,
            _ => ToolKind::Other,
        }
    }
}
```

## Implementation Notes
Add tool kind classification comments:
```rust
// ACP tool kinds enable client UI optimization and user experience:
// 1. read: File/data reading operations - clients can show read icons
// 2. edit: Content modification - clients can highlight changes
// 3. delete: Removal operations - clients can show warning indicators
// 4. move: File movement/rename - clients can track location changes  
// 5. search: Information discovery - clients can show search progress
// 6. execute: Command execution - clients can show terminal-like UI
// 7. think: Agent reasoning - clients can show thinking indicators
// 8. fetch: External data retrieval - clients can show network activity
// 9. other: Fallback for unclassified tools
//
// Proper classification improves client user experience and tool visibility.
```

### Tool Kind Detection Strategies
- [ ] Analyze tool name patterns for automatic classification
- [ ] Examine tool parameters to infer operation type
- [ ] Use tool description metadata for classification hints
- [ ] Support tool developer-specified kind annotations
- [ ] Add machine learning-based kind prediction for unknown tools

### Tool Kind Validation and Consistency
- [ ] Validate tool kind assignments against actual tool behavior
- [ ] Ensure tool kind consistency across tool call updates
- [ ] Add tool kind conflict detection and resolution
- [ ] Support tool kind auditing and reporting
- [ ] Implement tool kind compliance checking

### Configuration and Customization
- [ ] Add configurable tool kind mapping rules
- [ ] Support custom tool kind definitions for specific tools
- [ ] Allow tool kind override based on context or parameters
- [ ] Add tool kind configuration validation
- [ ] Support runtime tool kind mapping updates

### Client Integration Hints
- [ ] Document tool kind usage for client developers
- [ ] Provide tool kind to UI element mapping guidelines
- [ ] Add tool kind icon and color recommendations
- [ ] Support tool kind grouping and categorization
- [ ] Include tool kind in client capability negotiations

## Tool Kind Classification Examples
```rust
impl ToolKindClassifier {
    pub fn classify_tool(&self, tool: &Tool) -> ToolKind {
        // Check explicit tool kind annotation
        if let Some(kind) = tool.declared_kind() {
            return kind;
        }
        
        // Classify by tool name patterns
        if let Some(kind) = ToolKind::from_tool_name(&tool.name) {
            return kind;
        }
        
        // Classify by operation type
        if let Some(operation) = tool.operation_type() {
            return ToolKind::from_operation(&operation);
        }
        
        // Classify by parameters and context
        if let Some(kind) = self.classify_by_parameters(&tool.parameters) {
            return kind;
        }
        
        // Default fallback
        ToolKind::Other
    }
}
```

## Testing Requirements
- [ ] Test tool kind classification for all ACP-defined kinds
- [ ] Test automatic kind detection based on tool names
- [ ] Test kind inference from tool operations and parameters
- [ ] Test tool kind validation and consistency checking
- [ ] Test tool kind serialization and deserialization
- [ ] Test tool kind configuration and mapping rules
- [ ] Test tool kind integration with tool call reporting
- [ ] Test edge cases and fallback to `other` kind

## Integration Points
- [ ] Connect to tool execution and reporting system
- [ ] Integrate with tool registration and discovery
- [ ] Connect to tool call status and update mechanisms
- [ ] Integrate with client capability and UI systems

## Performance Considerations
- [ ] Optimize tool kind classification overhead
- [ ] Cache tool kind assignments for repeated tools
- [ ] Support batch tool kind classification
- [ ] Minimize classification impact on tool execution

## Acceptance Criteria
- Complete `ToolKind` enum with all ACP-specified variants
- Automatic tool kind classification based on tool name and operation
- Tool kind integration with tool call reporting system
- Configurable tool kind mapping rules and overrides
- Tool kind validation and consistency checking
- Documentation and guidelines for client UI optimization
- Complete test coverage for all tool kind scenarios
- Performance optimization for classification overhead
- Integration with existing tool execution and reporting systems