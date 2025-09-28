# Implement Tool Call Partial Update Optimization

## Problem
Our tool call update system doesn't implement the partial update optimization required by the ACP specification. According to the spec, "only the fields being changed need to be included" in tool call updates, but our implementation may send complete objects unnecessarily.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/tool-calls:

**Partial Update Principle:**
> "All fields except `toolCallId` are optional in updates. Only the fields being changed need to be included."

**Example Efficient Updates:**
```json
// Status-only update
{
  "sessionUpdate": "tool_call_update",
  "toolCallId": "call_001",
  "status": "in_progress"
}
```

```json
// Content-only update  
{
  "sessionUpdate": "tool_call_update",
  "toolCallId": "call_001",
  "content": [{"type": "content", "content": {"type": "text", "text": "Progress: 50%"}}]
}
```

```json
// Multiple field update
{
  "sessionUpdate": "tool_call_update", 
  "toolCallId": "call_001",
  "status": "completed",
  "content": [{"type": "content", "content": {"type": "text", "text": "Analysis complete"}}]
}
```

## Current Issues
- Tool call updates may send complete objects instead of partial changes
- No field-level change detection and tracking
- Missing optimization for bandwidth and processing efficiency
- No support for incremental updates during tool execution

## Implementation Tasks

### Partial Update Data Structure
- [ ] Define `ToolCallUpdate` struct with optional fields
- [ ] Implement partial update serialization with field omission
- [ ] Add update field validation and consistency checking
- [ ] Support update merging and field precedence rules

### Change Detection System
- [ ] Implement field-level change detection for tool calls
- [ ] Track previous tool call state for comparison
- [ ] Generate minimal update payloads with only changed fields
- [ ] Add change detection optimization for common update patterns

### Update Builder Pattern
- [ ] Create tool call update builder for incremental construction
- [ ] Support fluent API for adding changed fields
- [ ] Add update validation before sending
- [ ] Implement update batching for multiple simultaneous changes

### Field-Specific Update Optimization
- [ ] Optimize status-only updates (most common case)
- [ ] Handle content streaming with incremental content updates
- [ ] Support location updates without full tool call state
- [ ] Optimize raw input/output updates independently

## Partial Update Implementation
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallUpdate {
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ToolKind>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ToolCallStatus>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ToolCallContent>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<ToolCallLocation>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<serde_json::Value>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<serde_json::Value>,
}

impl ToolCallUpdate {
    pub fn status_only(tool_call_id: String, status: ToolCallStatus) -> Self {
        Self {
            tool_call_id,
            status: Some(status),
            title: None,
            kind: None,
            content: None,
            locations: None,
            raw_input: None,
            raw_output: None,
        }
    }
    
    pub fn content_only(tool_call_id: String, content: Vec<ToolCallContent>) -> Self {
        Self {
            tool_call_id,
            content: Some(content),
            title: None,
            kind: None,
            status: None,
            locations: None,
            raw_input: None,
            raw_output: None,
        }
    }
}
```

## Implementation Notes
Add partial update optimization comments:
```rust
// ACP partial updates optimize bandwidth and processing efficiency:
// 1. Only send changed fields to reduce payload size
// 2. Status-only updates for simple progress reporting
// 3. Content-only updates for streaming output
// 4. Field omission using Option<T> with skip_serializing_if
// 5. Change detection to generate minimal update payloads
//
// Partial updates improve performance and client responsiveness.
```

### Update Builder Pattern
```rust
pub struct ToolCallUpdateBuilder {
    tool_call_id: String,
    previous_state: Option<ToolCallState>,
    changes: ToolCallUpdate,
}

impl ToolCallUpdateBuilder {
    pub fn new(tool_call_id: String) -> Self {
        Self {
            tool_call_id: tool_call_id.clone(),
            previous_state: None,
            changes: ToolCallUpdate::empty(tool_call_id),
        }
    }
    
    pub fn with_status(mut self, status: ToolCallStatus) -> Self {
        self.changes.status = Some(status);
        self
    }
    
    pub fn add_content(mut self, content: ToolCallContent) -> Self {
        self.changes.content.get_or_insert_with(Vec::new).push(content);
        self
    }
    
    pub fn build(self) -> ToolCallUpdate {
        self.changes
    }
}
```

### Change Detection Engine
- [ ] Compare current tool call state with previous state
- [ ] Detect field-level changes efficiently
- [ ] Generate optimal update payloads
- [ ] Support deep change detection for nested structures
- [ ] Add change detection caching for performance

### Update Streaming and Batching
- [ ] Support streaming updates for long-running tools
- [ ] Batch multiple rapid updates into single payload
- [ ] Add update throttling to prevent excessive notifications
- [ ] Implement update queue management
- [ ] Support update prioritization by field importance

### Content Update Optimization
- [ ] Support incremental content updates (append vs replace)
- [ ] Optimize content streaming with partial payloads
- [ ] Add content deduplication for repeated updates
- [ ] Handle large content updates efficiently
- [ ] Support content update compression

### Performance Measurements
- [ ] Measure update payload size reduction
- [ ] Track update frequency and patterns
- [ ] Monitor change detection overhead
- [ ] Add update optimization metrics
- [ ] Support performance analysis and tuning

## Testing Requirements
- [ ] Test partial updates only include changed fields
- [ ] Test change detection accuracy for all field types
- [ ] Test update builder pattern for different scenarios
- [ ] Test update payload size optimization
- [ ] Test concurrent updates and change detection
- [ ] Test update streaming and batching performance
- [ ] Test field omission in serialization
- [ ] Test update merging and precedence rules

## Integration Points
- [ ] Connect to existing tool call tracking system
- [ ] Integrate with session update notification system
- [ ] Connect to tool execution monitoring
- [ ] Integrate with performance monitoring and metrics

## Backward Compatibility
- [ ] Ensure partial updates work with existing clients
- [ ] Support clients expecting complete tool call objects
- [ ] Add compatibility flags for update behavior
- [ ] Test interoperability with different client versions

## Acceptance Criteria
- Partial tool call updates with only changed fields included
- Field-level change detection and optimization
- Update builder pattern for incremental update construction
- Significant reduction in update payload sizes
- Performance optimization for high-frequency updates
- Integration with existing tool call reporting system
- Backward compatibility with existing client implementations
- Comprehensive test coverage for all update scenarios
- Performance metrics and monitoring integration
- Documentation of optimization benefits and usage patterns