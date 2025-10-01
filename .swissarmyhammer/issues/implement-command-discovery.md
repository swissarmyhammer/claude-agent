# Implement Command Discovery System

## Description
Implement comprehensive command discovery from multiple sources: MCP servers, tool handler capabilities, and permission engine.

## Locations
- `lib/src/agent.rs:1888` - MCP server commands
- `lib/src/agent.rs:1889` - Tool handler commands based on capabilities
- `lib/src/agent.rs:1890` - Permission engine commands (available vs restricted)

## Code Context
```rust
// TODO: Add commands from MCP servers for this session
// TODO: Add commands from tool handler based on capabilities
// TODO: Add commands from permission engine (available vs restricted)
```

## Implementation Notes
- Create unified command discovery interface
- Aggregate commands from all three sources
- Handle conflicts and priorities between command sources
- Filter commands based on permissions