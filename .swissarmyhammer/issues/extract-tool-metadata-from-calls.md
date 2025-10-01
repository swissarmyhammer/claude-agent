# Extract Tool Metadata from Tool Calls

## Description
Implement proper extraction of tool name and arguments from tool call objects instead of using placeholder values.

## Locations
- `lib/src/agent.rs:3133` - Tool name extraction
- `lib/src/agent.rs:3134` - Tool arguments extraction

## Code Context
```rust
let tool_name = "unknown_tool"; // TODO: Extract from tool_call
let tool_args = serde_json::json!({}); // TODO: Extract from tool_call
```

## Implementation Notes
- Parse tool_call structure to extract name
- Extract and validate tool arguments
- Handle malformed tool calls gracefully
- Add error handling for missing fields