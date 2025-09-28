# Implement File Location Tracking for Follow-Along Features

## Problem
Our tool call reporting doesn't implement file location tracking required by the ACP specification. This prevents clients from implementing "follow-along" features that track which files the agent is accessing or modifying in real-time.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/tool-calls:

**File Location Tracking Structure:**
```json
{
  "sessionUpdate": "tool_call",
  "toolCallId": "call_001",
  "title": "Reading configuration file",
  "kind": "read",
  "status": "pending",
  "locations": [
    {"path": "/home/user/project/src/main.py", "line": 42},
    {"path": "/home/user/project/config/settings.json"}
  ]
}
```

**Location Structure:**
```json
{
  "path": "/home/user/project/src/main.py",
  "line": 42  // Optional line number
}
```

## Current Issues
- No file location tracking in tool call reports
- Missing integration with file operation detection
- No support for follow-along client features
- Missing location updates during tool execution

## Implementation Tasks

### File Location Data Structure
- [ ] Define `ToolCallLocation` struct with path and optional line number
- [ ] Add location validation and path normalization
- [ ] Support absolute path requirements
- [ ] Implement location serialization and deserialization
- [ ] Add location comparison and deduplication

### Location Tracking Integration
- [ ] Integrate location tracking with file operations
- [ ] Detect file access patterns during tool execution
- [ ] Track multiple file locations per tool call
- [ ] Support location updates throughout tool lifecycle
- [ ] Add location correlation with tool call status

### File Operation Detection
- [ ] Monitor file system operations during tool execution
- [ ] Detect file reads, writes, deletes, and moves
- [ ] Extract file paths from tool parameters and execution
- [ ] Support pattern-based file access detection
- [ ] Add file operation instrumentation

### Location Update Management
- [ ] Send location updates during tool execution progress
- [ ] Support incremental location additions
- [ ] Handle location changes during tool execution
- [ ] Add location removal for completed operations
- [ ] Implement location update batching for performance

## File Location Implementation
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCallLocation {
    pub path: String,
    pub line: Option<u32>,
}

impl ToolCallLocation {
    pub fn new(path: String) -> Self {
        Self { path, line: None }
    }
    
    pub fn with_line(path: String, line: u32) -> Self {
        Self { path, line: Some(line) }
    }
    
    pub fn validate_path(&self) -> Result<(), ValidationError> {
        // Ensure path is absolute
        if !self.path.starts_with('/') && !self.path.contains(':') {
            return Err(ValidationError::RelativePath(self.path.clone()));
        }
        
        // Normalize and validate path format
        Ok(())
    }
}

#[derive(Debug)]
pub struct LocationTracker {
    locations: Vec<ToolCallLocation>,
    file_monitor: FileMonitor,
}

impl LocationTracker {
    pub fn add_location(&mut self, location: ToolCallLocation);
    pub fn remove_location(&mut self, path: &str);
    pub fn get_locations(&self) -> &[ToolCallLocation];
    pub fn track_file_operation(&mut self, operation: &FileOperation);
}
```

## Implementation Notes
Add file location tracking comments:
```rust
// ACP file location tracking enables client follow-along features:
// 1. Real-time file access visibility for users
// 2. Client UI can highlight files being processed
// 3. Editor integration can show agent activity
// 4. File tree updates can reflect agent operations
// 5. Location updates provide progress transparency
//
// Location tracking enhances user awareness of agent file activity.
```

### File System Monitoring
- [ ] Implement file system event monitoring
- [ ] Track file access patterns during tool execution
- [ ] Detect file operations through system calls
- [ ] Monitor file descriptor usage
- [ ] Add file access logging and correlation

### Tool Integration Patterns
```rust
impl ToolExecutor {
    pub fn execute_with_location_tracking(&self, tool: &Tool) -> Result<ToolResult> {
        let mut location_tracker = LocationTracker::new();
        
        // Pre-execution: Extract known file paths from parameters
        self.extract_locations_from_parameters(tool, &mut location_tracker);
        
        // During execution: Monitor file system activity
        let result = self.execute_with_monitoring(tool, &mut location_tracker)?;
        
        // Post-execution: Report final locations
        self.report_final_locations(&location_tracker);
        
        Ok(result)
    }
    
    fn extract_locations_from_parameters(&self, tool: &Tool, tracker: &mut LocationTracker) {
        // Look for file path parameters
        if let Some(file_path) = tool.get_parameter("file_path") {
            tracker.add_location(ToolCallLocation::new(file_path));
        }
        
        // Extract paths from structured parameters
        self.extract_paths_recursively(&tool.parameters, tracker);
    }
}
```

### Location Update Strategies
- [ ] Send initial locations when tool call is created
- [ ] Update locations as tool execution progresses
- [ ] Add locations when new files are accessed
- [ ] Remove locations when operations complete
- [ ] Batch location updates for performance optimization

### Line Number Support
- [ ] Track specific line numbers for file operations
- [ ] Support line number extraction from tool operations
- [ ] Add line number validation and bounds checking
- [ ] Handle line number updates during file modifications
- [ ] Support line range tracking for multi-line operations

### Path Normalization and Security
- [ ] Normalize file paths to absolute format
- [ ] Validate paths are within allowed boundaries
- [ ] Sanitize paths to prevent security issues
- [ ] Handle symbolic links and path resolution
- [ ] Add path validation for cross-platform compatibility

## Testing Requirements
- [ ] Test location tracking for various file operations
- [ ] Test location updates throughout tool execution lifecycle
- [ ] Test line number tracking and validation
- [ ] Test path normalization and validation
- [ ] Test concurrent location tracking for multiple tools
- [ ] Test location deduplication and management
- [ ] Test integration with different tool types
- [ ] Test performance with large numbers of file operations

## Client Integration Benefits
- [ ] Enable real-time file activity visualization
- [ ] Support editor integration with agent activity highlighting
- [ ] Allow file tree updates reflecting agent operations
- [ ] Enable progress tracking based on file operations
- [ ] Support user awareness of agent file access patterns

## Integration Points
- [ ] Connect to file system operation detection
- [ ] Integrate with tool call reporting system
- [ ] Connect to tool execution monitoring
- [ ] Integrate with session update notifications

## Performance Considerations
- [ ] Optimize location tracking overhead
- [ ] Support efficient location deduplication
- [ ] Add location update batching
- [ ] Implement location tracking caching
- [ ] Monitor file system monitoring performance impact

## Acceptance Criteria
- Complete file location tracking in tool call reports
- Integration with file system operation detection
- Support for optional line number tracking
- Path normalization and validation for all locations
- Location updates throughout tool execution lifecycle
- Performance optimization for location tracking overhead
- Integration with existing tool call reporting system
- Client follow-along feature enablement
- Comprehensive test coverage for all location tracking scenarios
- Security validation for file path handling