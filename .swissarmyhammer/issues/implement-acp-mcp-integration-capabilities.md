# Implement ACP MCP Integration Capabilities

## Problem
Our initialization response doesn't properly declare MCP (Model Context Protocol) integration capabilities as required by the ACP specification. We need to explicitly declare support for HTTP MCP connections while excluding deprecated SSE support.

## Current Issues
- Missing MCP capabilities declaration in agent initialization response
- No clear indication of which MCP transport methods we support
- Potential capability negotiation failures with clients expecting MCP support

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/initialization:

**MCP Capabilities Structure:**
```json
{
  "agentCapabilities": {
    "mcp": {
      "http": true,
      "sse": false
    }
  }
}
```

## Implementation Decisions
Based on modern standards and MCP specification:
- **HTTP Support**: `true` - Standard, modern transport method
- **SSE Support**: `false` - Deprecated by MCP specification, not implemented

## Implementation Tasks
- [ ] Add `McpCapabilities` struct to capability types
- [ ] Include MCP capabilities in `AgentCapabilities` struct
- [ ] Set `http: true` in initialization response
- [ ] Set `sse: false` with explanatory comment
- [ ] Update initialization handler to populate MCP capabilities
- [ ] Add validation for MCP capability structure

## Required Code Comments
Add clear documentation about MCP transport decisions:

```rust
// MCP TRANSPORT DECISIONS:
// We support HTTP transport for MCP connections as it is the modern standard.
// SSE (Server-Sent Events) transport is explicitly set to false because:
// 1. SSE transport has been deprecated by the MCP specification
// 2. HTTP transport is more reliable and easier to implement
// 3. HTTP transport provides better error handling and connection management
//
// This is an architectural decision for simplicity and modern standards.
```

## MCP Integration Points
Ensure our MCP implementation aligns with declared capabilities:
- [ ] Verify existing MCP HTTP support works with capability declaration
- [ ] Ensure no SSE transport code is exposed
- [ ] Add capability validation for MCP method calls
- [ ] Document MCP integration architecture

## Testing Requirements
- [ ] Test MCP capabilities are properly declared in initialization
- [ ] Verify HTTP MCP connections work as declared
- [ ] Ensure SSE is properly disabled/unavailable
- [ ] Test capability negotiation with MCP-aware clients

## Acceptance Criteria
- MCP capabilities properly declared in initialization response
- HTTP transport capability set to `true`
- SSE transport capability set to `false` with explanatory comments
- MCP integration works according to declared capabilities
- Clear documentation of transport method decisions
- Tests verify complete MCP capability declaration
- No SSE transport functionality exposed