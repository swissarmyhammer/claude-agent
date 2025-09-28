# Implement Unsaved Editor State Integration

## Problem
Our file system implementation doesn't integrate with client editor state to access unsaved changes as specified in the ACP documentation. File reading should include unsaved editor modifications to provide accurate, real-time file content.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/file-system:

**Editor Integration Purpose:**
> "These methods enable Agents to access unsaved editor state and allow Clients to track file modifications made during agent execution."

**Key Requirements:**
- File reading should include unsaved changes from client editors
- Access to in-memory file buffers and modifications
- Real-time file content that reflects current editor state
- Integration with client workspace and editor management

## Current Issues
- No integration with client editor state for file reading
- Missing access to unsaved file modifications
- File operations may work on stale file content from disk
- No coordination with client workspace management

## Implementation Tasks

### Editor State Protocol Integration
- [ ] Design protocol for accessing client editor state
- [ ] Add editor buffer synchronization mechanisms
- [ ] Support real-time file content with unsaved changes
- [ ] Implement editor modification tracking and coordination

### File Content Resolution
- [ ] Prioritize editor buffer content over disk content
- [ ] Merge unsaved changes with disk content when appropriate
- [ ] Handle conflicts between editor state and disk state
- [ ] Support different editor buffer states (modified, clean, new)

### Client-Agent Editor Coordination
- [ ] Add editor state query mechanisms
- [ ] Implement editor buffer access protocols
- [ ] Support editor modification notifications
- [ ] Add editor state validation and consistency checking

### File Reading Enhancement
- [ ] Modify `fs/read_text_file` to include editor state
- [ ] Add editor buffer detection and access
- [ ] Support fallback to disk content when editor state unavailable
- [ ] Implement editor state caching and optimization

## Editor State Implementation
```rust
pub struct EditorStateManager {
    client_connection: Arc<ClientConnection>,
    buffer_cache: Arc<Mutex<HashMap<String, EditorBuffer>>>,
}

#[derive(Debug, Clone)]
pub struct EditorBuffer {
    pub path: String,
    pub content: String,
    pub modified: bool,
    pub last_modified: SystemTime,
    pub encoding: String,
}

impl EditorStateManager {
    pub async fn get_file_content_with_editor_state(
        &self,
        session_id: &str,
        path: &str,
    ) -> Result<String, EditorStateError> {
        // First try to get content from editor buffer
        if let Some(buffer_content) = self.get_editor_buffer_content(session_id, path).await? {
            return Ok(buffer_content);
        }
        
        // Fallback to disk content if no editor buffer
        let disk_content = tokio::fs::read_to_string(path).await
            .map_err(|e| EditorStateError::DiskReadFailed(path.to_string(), e))?;
        
        Ok(disk_content)
    }
    
    async fn get_editor_buffer_content(
        &self,
        session_id: &str,
        path: &str,
    ) -> Result<Option<String>, EditorStateError> {
        // Check cache first
        if let Some(cached) = self.get_cached_buffer(path).await {
            return Ok(Some(cached.content));
        }
        
        // Query client for editor buffer state
        self.query_client_editor_buffer(session_id, path).await
    }
}
```

## Implementation Notes
Add editor state integration comments:
```rust
// ACP requires integration with client editor state:
// 1. File reading should include unsaved editor changes
// 2. Access in-memory buffers before falling back to disk
// 3. Coordinate with client workspace and editor management
// 4. Handle conflicts between editor state and disk content
// 5. Support real-time file content that reflects current editor state
//
// Editor integration ensures agents work with current, not stale, file content.
```

### Client Protocol Extension
- [ ] Design client-agent communication for editor state queries
- [ ] Add editor buffer request/response protocols
- [ ] Implement editor state notification system
- [ ] Support batch editor buffer queries for performance

### Buffer State Management
```rust
#[derive(Debug)]
pub enum EditorBufferState {
    Clean,           // Buffer matches disk content
    Modified,        // Buffer has unsaved changes
    New,             // File exists only in editor, not on disk
    Conflicted,      // Editor and disk content differ
}

impl EditorBuffer {
    pub fn get_effective_content(&self) -> &str {
        match self.state() {
            EditorBufferState::Clean => &self.content,
            EditorBufferState::Modified => &self.content,
            EditorBufferState::New => &self.content,
            EditorBufferState::Conflicted => {
                // Policy decision: prefer editor content
                &self.content
            }
        }
    }
}
```

### File Write Coordination
- [ ] Notify client of file modifications made by agent
- [ ] Update editor buffers when agent writes files
- [ ] Handle editor buffer invalidation after writes
- [ ] Support editor buffer refresh and synchronization

### Workspace Integration
- [ ] Integrate with client workspace management
- [ ] Support workspace-wide file tracking
- [ ] Add project-level file operation coordination
- [ ] Handle workspace file organization and structure

### Performance and Caching
- [ ] Cache editor buffer content for repeated access
- [ ] Optimize editor state queries for performance
- [ ] Support incremental buffer updates
- [ ] Add buffer state invalidation and refresh mechanisms

### Error Handling
- [ ] Handle editor state query failures gracefully
- [ ] Fallback to disk content when editor state unavailable
- [ ] Handle editor buffer conflicts and inconsistencies
- [ ] Add proper error responses for editor state failures

## Client Integration Protocol
```rust
// Proposed extension for editor state access
pub struct EditorStateRequest {
    pub session_id: String,
    pub paths: Vec<String>,
}

pub struct EditorStateResponse {
    pub buffers: Vec<EditorBuffer>,
    pub unavailable_paths: Vec<String>,
}

impl ClientConnection {
    pub async fn query_editor_buffers(
        &self,
        request: EditorStateRequest,
    ) -> Result<EditorStateResponse, ClientError> {
        // Send request to client for editor buffer state
        // This would require extending the ACP protocol or using custom extensions
    }
}
```

## Testing Requirements
- [ ] Test file reading with and without unsaved editor changes
- [ ] Test editor buffer priority over disk content
- [ ] Test fallback to disk content when editor state unavailable
- [ ] Test editor buffer caching and invalidation
- [ ] Test file write coordination with editor buffer updates
- [ ] Test workspace integration and file tracking
- [ ] Test error handling for editor state failures
- [ ] Test performance with large numbers of editor buffers

## Integration Points
- [ ] Connect to client communication and protocol systems
- [ ] Integrate with existing file system method handlers
- [ ] Connect to session management and validation
- [ ] Integrate with workspace and project management

## Configuration and Policy
- [ ] Add configurable editor state integration policies
- [ ] Support different editor buffer priority strategies
- [ ] Configure editor state caching and refresh rates
- [ ] Add editor state query timeout and fallback policies

## Future Considerations
- [ ] Consider ACP protocol extension for editor state access
- [ ] Design backward compatibility with clients not supporting editor state
- [ ] Plan for different client editor architectures
- [ ] Support multiple editor instances and workspaces

## Acceptance Criteria
- File reading includes unsaved editor changes when available
- Graceful fallback to disk content when editor state unavailable
- Editor buffer caching and performance optimization
- Integration with client workspace and editor management
- File write coordination with editor buffer updates
- Proper error handling for editor state failures
- Configuration support for editor integration policies
- Comprehensive test coverage for all editor state scenarios
- Documentation of editor integration requirements and behavior
- Future-proof design for ACP protocol extensions