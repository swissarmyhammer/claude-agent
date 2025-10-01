# Implement SSE MCP Transport

## Description
Implement full Server-Sent Events (SSE) transport for MCP protocol. Currently returns empty tools list as placeholder.

## Locations
- `lib/src/mcp.rs:483-484` - SSE transport implementation
- `lib/src/mcp.rs:498` - SSE tools list

## Code Context
```rust
// For now, return empty tools list as SSE implementation is complex
// This is a placeholder for the full SSE implementation

// For now, return empty list as we'll get tools from tools/list call
```

## Implementation Notes
- Implement SSE connection handling
- Add event stream parsing
- Implement tools/list protocol over SSE
- Add reconnection logic
- Handle streaming errors
- Test with real SSE MCP servers