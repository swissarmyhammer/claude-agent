# Implement Complete Tool Call Reporting Structure

## Problem
Our tool call reporting doesn't implement the complete structure required by the ACP specification. We need comprehensive tool call metadata including kinds, locations, raw input/output, and proper status lifecycle management.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/tool-calls:

**Complete Tool Call Structure:**
```json
{
  "sessionUpdate": "tool_call",
  "toolCallId": "call_001",
  "title": "Reading configuration file",
  "kind": "read",
  "status": "pending",
  "content": [],
  "locations": [
    {"path": "/home/user/project/config.json", "line": 42}
  ],
  "rawInput": {"file_path": "/path/to/file", "mode": "read"},
  "rawOutput": {"content": "file contents...", "size": 1024}
}
```

## Current Issues
- Tool call reporting may not include all metadata fields
- Missing tool kind classification and reporting
- No file location tracking for follow-along features
- Raw input/output capture and reporting unclear
- Incomplete tool call metadata structure

## Implementation Tasks

### Tool Call Data Structure
- [ ] Define complete `ToolCall` struct with all ACP-required fields
- [ ] Implement `toolCallId` unique identifier generation
- [ ] Add human-readable `title` field for tool descriptions
- [ ] Include `kind` field for tool classification
- [ ] Add `status` field for lifecycle management
- [ ] Support `content`, `locations`, `rawInput`, `rawOutput` arrays

### Tool Call ID Management
- [ ] Implement unique tool call ID generation
- [ ] Ensure tool call IDs are unique within session scope
- [ ] Add tool call ID validation and format consistency
- [ ] Support tool call ID correlation across updates
- [ ] Handle tool call ID conflicts and collision detection

### Tool Title Generation
- [ ] Generate descriptive, human-readable tool titles
- [ ] Create context-aware titles based on tool parameters
- [ ] Support localization for tool titles
- [ ] Add title templates for common tool operations
- [ ] Implement dynamic title updates based on progress

### Tool Call Content Management
- [ ] Support content arrays in tool call reports
- [ ] Handle different content types (text, images, etc.)
- [ ] Add content streaming during tool execution
- [ ] Support content updates throughout tool lifecycle
- [ ] Implement content size limits and pagination

### Raw Input/Output Tracking
- [ ] Capture and store raw tool input parameters
- [ ] Record raw tool output data and results
- [ ] Support structured data in raw I/O fields
- [ ] Add raw data sanitization for security
- [ ] Implement raw data size limits and truncation

## Tool Call Reporting Implementation
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallReport {
    pub tool_call_id: String,
    pub title: String,
    pub kind: ToolKind,
    pub status: ToolCallStatus,
    pub content: Vec<ToolCallContent>,
    pub locations: Vec<ToolCallLocation>,
    pub raw_input: Option<serde_json::Value>,
    pub raw_output: Option<serde_json::Value>,
}

impl ToolCallReport {
    pub fn new(id: String, title: String, kind: ToolKind) -> Self;
    pub fn update_status(&mut self, status: ToolCallStatus);
    pub fn add_content(&mut self, content: ToolCallContent);
    pub fn add_location(&mut self, location: ToolCallLocation);
    pub fn set_raw_input(&mut self, input: serde_json::Value);
    pub fn set_raw_output(&mut self, output: serde_json::Value);
}
```

## Implementation Notes
Add tool call reporting comments:
```rust
// ACP requires comprehensive tool call reporting with rich metadata:
// 1. toolCallId: Unique identifier for correlation across updates
// 2. title: Human-readable description of tool operation
// 3. kind: Classification for UI optimization and icon selection
// 4. status: Lifecycle state (pending, in_progress, completed, failed)
// 5. content: Output content produced by tool execution
// 6. locations: File paths for follow-along features
// 7. rawInput/rawOutput: Detailed I/O data for debugging
//
// Complete reporting enables rich client experiences and debugging.
```

### Tool Call Location Tracking
- [ ] Track file paths accessed or modified by tools
- [ ] Include optional line numbers for precise location tracking
- [ ] Support multiple locations per tool call
- [ ] Add location validation and path normalization
- [ ] Implement location updates during tool execution

### Metadata Enrichment
- [ ] Add tool execution timestamps
- [ ] Include resource usage metrics
- [ ] Track tool execution duration
- [ ] Add dependency information between tool calls
- [ ] Support custom metadata fields for specific tools

### Integration with Session Updates
- [ ] Send initial tool call notification with complete structure
- [ ] Support partial updates with changed fields only
- [ ] Ensure proper notification ordering and correlation
- [ ] Handle concurrent tool call reporting
- [ ] Add tool call aggregation for batch operations

### Performance Optimization
- [ ] Optimize tool call metadata capture overhead
- [ ] Support streaming updates for long-running tools
- [ ] Add memory management for tool call history
- [ ] Implement efficient tool call lookup and correlation
- [ ] Support tool call data compression for large payloads

## Testing Requirements
- [ ] Test complete tool call structure serialization
- [ ] Test tool call ID uniqueness and correlation
- [ ] Test tool title generation for different tool types
- [ ] Test location tracking with file operations
- [ ] Test raw input/output capture and reporting
- [ ] Test tool call metadata updates throughout lifecycle
- [ ] Test concurrent tool call reporting
- [ ] Test tool call memory and performance impact

## Integration Points
- [ ] Connect to existing tool execution system
- [ ] Integrate with session update notification system
- [ ] Connect to file operation tracking
- [ ] Integrate with tool call status management

## Error Handling
- [ ] Handle missing or invalid tool call metadata
- [ ] Validate tool call structure before reporting
- [ ] Handle tool call correlation failures
- [ ] Support graceful degradation for optional fields
- [ ] Add error recovery for metadata capture failures

## Acceptance Criteria
- Complete tool call reporting structure matching ACP specification
- Unique tool call ID generation and correlation
- Human-readable tool titles with context awareness
- File location tracking for follow-along features
- Raw input/output capture with security sanitization
- Integration with existing tool execution and notification systems
- Performance optimization for tool call metadata overhead
- Comprehensive test coverage for all reporting scenarios
- Proper error handling for metadata capture failures