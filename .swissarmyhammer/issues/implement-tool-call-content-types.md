# Implement Tool Call Content Types

## Problem
Our tool call content reporting doesn't support all three content types required by the ACP specification. We need to implement regular content, diffs, and terminal embedding to provide complete tool call output reporting.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/tool-calls:

**Three Content Types Required:**

**1. Regular Content:**
```json
{
  "type": "content",
  "content": {
    "type": "text",
    "text": "Analysis complete. Found 3 issues."
  }
}
```

**2. Diffs:**
```json
{
  "type": "diff",
  "path": "/home/user/project/src/config.json",
  "oldText": "{\n  \"debug\": false\n}",
  "newText": "{\n  \"debug\": true\n}"
}
```

**3. Terminals:**
```json
{
  "type": "terminal",
  "terminalId": "term_xyz789"
}
```

## Current Issues
- Tool call content types may not support all variants
- Missing diff generation and reporting for file modifications
- No terminal integration with tool call content
- Missing content type validation and handling

## Implementation Tasks

### Tool Call Content Type Infrastructure
- [ ] Define `ToolCallContent` enum with all three content variants
- [ ] Implement proper serialization/deserialization for content types
- [ ] Add content type validation and consistency checking
- [ ] Support content type conversion and formatting

### Regular Content Support
- [ ] Implement standard content block embedding in tool calls
- [ ] Support all ACP content block types (text, image, audio, resource, resource_link)
- [ ] Add content block validation for tool call context
- [ ] Handle content streaming and updates during tool execution

### Diff Content Implementation
- [ ] Generate file modification diffs for edit operations
- [ ] Support unified diff format with old/new text comparison
- [ ] Add diff validation and format checking
- [ ] Implement diff generation from file operations
- [ ] Support binary file diff handling

### Terminal Content Integration
- [ ] Embed terminal sessions in tool call content
- [ ] Integrate with terminal creation and lifecycle management
- [ ] Support live terminal output streaming in tool calls
- [ ] Handle terminal persistence after tool completion
- [ ] Add terminal session correlation and management

## Tool Call Content Implementation
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    Content {
        content: ContentBlock,
    },
    Diff {
        path: String,
        old_text: Option<String>,
        new_text: String,
    },
    Terminal {
        terminal_id: String,
    },
}

impl ToolCallContent {
    pub fn from_content_block(content: ContentBlock) -> Self {
        Self::Content { content }
    }
    
    pub fn from_file_diff(path: String, old_text: Option<String>, new_text: String) -> Self {
        Self::Diff { path, old_text, new_text }
    }
    
    pub fn from_terminal(terminal_id: String) -> Self {
        Self::Terminal { terminal_id }
    }
}
```

## Implementation Notes
Add tool call content types comments:
```rust
// ACP requires three distinct tool call content types:
// 1. Content: Standard content blocks (text, images, resources, etc.)
// 2. Diff: File modification diffs showing old vs new content
// 3. Terminal: Embedded terminal sessions with live output
//
// Different content types enable rich tool call output presentation.
```

### Diff Generation System
- [ ] Implement file content comparison and diff generation
- [ ] Support different diff formats (unified, context, side-by-side)
- [ ] Add diff optimization for large files
- [ ] Handle binary file differences
- [ ] Support diff highlighting and formatting

### File Operation Integration
```rust
impl FileOperationHandler {
    pub fn generate_diff_content(&self, operation: &FileOperation) -> Option<ToolCallContent> {
        match operation {
            FileOperation::Write { path, old_content, new_content } => {
                Some(ToolCallContent::from_file_diff(
                    path.clone(),
                    old_content.clone(),
                    new_content.clone(),
                ))
            }
            FileOperation::Edit { path, changes } => {
                let (old_text, new_text) = self.apply_changes(path, changes)?;
                Some(ToolCallContent::from_file_diff(
                    path.clone(),
                    Some(old_text),
                    new_text,
                ))
            }
            _ => None,
        }
    }
}
```

### Terminal Integration
- [ ] Connect to terminal protocol implementation
- [ ] Handle terminal creation and lifecycle in tool calls
- [ ] Support terminal output streaming during tool execution
- [ ] Add terminal session cleanup and resource management
- [ ] Implement terminal content persistence

### Content Type Selection Logic
- [ ] Automatically select appropriate content type based on tool operation
- [ ] Use diffs for file modification operations
- [ ] Use terminals for command execution operations
- [ ] Use regular content for analysis and read operations
- [ ] Support manual content type override for specific tools

### Content Validation and Security
- [ ] Validate content type structure and required fields
- [ ] Sanitize diff content to prevent security issues
- [ ] Validate terminal IDs and session correlation
- [ ] Add content size limits for different content types
- [ ] Implement content type security filtering

## Testing Requirements
- [ ] Test all three content types serialization and deserialization
- [ ] Test diff generation for file modification operations
- [ ] Test terminal integration with tool call content
- [ ] Test regular content block embedding in tool calls
- [ ] Test content type validation and error handling
- [ ] Test content streaming and updates during tool execution
- [ ] Test content type selection logic for different tools
- [ ] Test content security and sanitization measures

## Integration Points
- [ ] Connect to file operation and diff generation systems
- [ ] Integrate with terminal protocol implementation
- [ ] Connect to content block processing system
- [ ] Integrate with tool call reporting and status updates

### Performance Considerations
- [ ] Optimize diff generation for large files
- [ ] Support streaming content updates for long-running tools
- [ ] Add content caching and deduplication
- [ ] Implement efficient content type conversion
- [ ] Support content compression for large outputs

## Client Integration Guidelines
- [ ] Document content type usage for client developers
- [ ] Provide content type display recommendations
- [ ] Add content type UI integration examples
- [ ] Support content type accessibility features
- [ ] Include content type in client capability negotiations

## Acceptance Criteria
- Complete support for all three ACP tool call content types
- Automatic diff generation for file modification operations
- Terminal integration with live output streaming
- Regular content block support for all ACP content types
- Content type validation and security measures
- Automatic content type selection based on tool operation
- Integration with existing tool call reporting system
- Performance optimization for large content and streaming
- Comprehensive test coverage for all content type scenarios
- Client integration guidelines and documentation