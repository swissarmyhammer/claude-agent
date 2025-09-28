# Implement Complete fs/read_text_file Method

## Problem
Our file reading implementation may not support all parameters required by the ACP specification. We need complete support for the `fs/read_text_file` method including optional line offset and limit parameters for partial file reading.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/file-system:

**Complete Method Signature:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "fs/read_text_file",
  "params": {
    "sessionId": "sess_abc123def456",
    "path": "/home/user/project/src/main.py",
    "line": 10,    // Optional: 1-based line number to start from
    "limit": 50    // Optional: maximum number of lines to read
  }
}
```

**Response Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": "def hello_world():\n    print('Hello, world!')\n"
  }
}
```

## Current Issues
- May not support optional `line` parameter for starting line offset
- May not support optional `limit` parameter for maximum lines to read
- Missing partial file reading optimization for large files
- Line counting and offset calculation unclear

## Implementation Tasks

### Method Handler Implementation
- [ ] Implement complete `fs/read_text_file` method handler
- [ ] Add proper JSON-RPC method registration
- [ ] Support all required and optional parameters
- [ ] Add request parameter validation and error handling

### Parameter Support
- [ ] Support required `sessionId` parameter with validation
- [ ] Support required `path` parameter with absolute path validation
- [ ] Support optional `line` parameter (1-based line offset)
- [ ] Support optional `limit` parameter (maximum lines to read)
- [ ] Add parameter combination validation and edge case handling

### File Reading Logic
- [ ] Implement full file reading when no line/limit specified
- [ ] Add line offset calculation for `line` parameter
- [ ] Implement line limit enforcement for `limit` parameter
- [ ] Support efficient partial file reading for large files

### Line-Based File Processing
- [ ] Implement 1-based line numbering (per ACP specification)
- [ ] Add line counting and offset calculation
- [ ] Support different line ending formats (LF, CRLF, CR)
- [ ] Handle edge cases (empty files, single line files, etc.)

## File Reading Implementation
```rust
#[derive(Debug, Deserialize)]
pub struct ReadTextFileParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub path: String,
    pub line: Option<u32>,  // 1-based line number
    pub limit: Option<u32>, // maximum lines to read
}

#[derive(Debug, Serialize)]
pub struct ReadTextFileResponse {
    pub content: String,
}

pub async fn handle_read_text_file(
    params: ReadTextFileParams
) -> Result<ReadTextFileResponse, FileSystemError> {
    // Validate session ID
    validate_session_id(&params.session_id)?;
    
    // Validate absolute path
    validate_absolute_path(&params.path)?;
    
    // Read file content with line offset and limit
    let content = read_file_with_options(
        &params.path,
        params.line,
        params.limit
    ).await?;
    
    Ok(ReadTextFileResponse { content })
}

async fn read_file_with_options(
    path: &str,
    start_line: Option<u32>,
    limit: Option<u32>
) -> Result<String, FileSystemError> {
    let file_content = tokio::fs::read_to_string(path).await
        .map_err(|e| FileSystemError::ReadFailed(path.to_string(), e))?;
    
    // Apply line offset and limit if specified
    apply_line_filtering(&file_content, start_line, limit)
}
```

## Implementation Notes
Add file reading method comments:
```rust
// ACP fs/read_text_file method implementation:
// 1. sessionId: Required - validate against active sessions
// 2. path: Required - must be absolute path
// 3. line: Optional - 1-based line number to start reading from
// 4. limit: Optional - maximum number of lines to read
// 5. Response: content field with requested file content
//
// Supports partial file reading for performance optimization.
```

### Line Filtering Implementation
```rust
fn apply_line_filtering(
    content: &str,
    start_line: Option<u32>,
    limit: Option<u32>
) -> Result<String, FileSystemError> {
    let lines: Vec<&str> = content.lines().collect();
    
    let start_index = match start_line {
        Some(line) if line > 0 => (line - 1) as usize, // Convert to 0-based
        Some(_) => return Err(FileSystemError::InvalidLineNumber),
        None => 0,
    };
    
    if start_index >= lines.len() {
        return Ok(String::new()); // Past end of file
    }
    
    let end_index = match limit {
        Some(limit_count) => std::cmp::min(
            start_index + limit_count as usize,
            lines.len()
        ),
        None => lines.len(),
    };
    
    let selected_lines = &lines[start_index..end_index];
    Ok(selected_lines.join("\n"))
}
```

### Editor State Integration
- [ ] Access unsaved editor changes from client
- [ ] Integrate with client workspace state
- [ ] Handle files that exist only in memory
- [ ] Support real-time file content with modifications

### Error Handling
- [ ] Handle file not found errors with proper ACP codes
- [ ] Handle permission denied errors
- [ ] Handle invalid path format errors
- [ ] Handle invalid line number errors
- [ ] Handle large file reading timeouts

### Performance Optimization
- [ ] Optimize large file reading with streaming
- [ ] Add file size limits and validation
- [ ] Support memory-efficient partial reading
- [ ] Add file reading caching where appropriate

## Testing Requirements
- [ ] Test full file reading without line/limit parameters
- [ ] Test partial reading with line offset parameter
- [ ] Test partial reading with limit parameter
- [ ] Test combination of line offset and limit parameters
- [ ] Test edge cases (empty files, single line, past EOF)
- [ ] Test different line ending formats
- [ ] Test error scenarios (file not found, permissions, etc.)
- [ ] Test performance with large files

## Integration Points
- [ ] Connect to session validation system
- [ ] Integrate with client capability validation
- [ ] Connect to path validation and security systems
- [ ] Integrate with editor state and workspace management

## Security Considerations
- [ ] Validate paths are within allowed boundaries
- [ ] Prevent path traversal attacks
- [ ] Add file size limits to prevent DoS
- [ ] Implement access control based on session context

## Acceptance Criteria
- Complete `fs/read_text_file` method handler with all parameters
- Support for optional `line` parameter with 1-based line numbering
- Support for optional `limit` parameter with line count limits
- Proper error handling for all failure scenarios
- Integration with session validation and capability checking
- Performance optimization for large file and partial reading
- Comprehensive test coverage for all parameter combinations
- Security validation for file access and path boundaries
- Documentation of method behavior and parameter usage