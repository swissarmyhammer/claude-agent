# Implement Complete Terminal Output Management System

## Problem
Our terminal implementation may not support real-time output retrieval and management as required by the ACP specification. We need comprehensive output capture, byte limit enforcement, truncation management, and exit status reporting.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/terminals:

**terminal/output Method:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "terminal/output",
  "params": {
    "sessionId": "sess_abc123def456",
    "terminalId": "term_xyz789"
  }
}
```

**Response Format:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": {
    "output": "Running tests...\n✓ All tests passed (42 total)\n",
    "truncated": false,
    "exitStatus": {
      "exitCode": 0,
      "signal": null
    }
  }
}
```

**Output Management Requirements:**
- Real-time output capture from stdout/stderr
- Output byte limit enforcement with truncation from beginning
- Character boundary truncation to maintain valid strings
- Exit status reporting when process completes

## Current Issues
- Real-time output capture and retrieval unclear
- Output byte limit and truncation implementation unclear
- Exit status tracking and reporting unclear
- Output buffer management and memory optimization unclear

## Implementation Tasks

### Output Capture System
- [ ] Implement real-time stdout/stderr capture
- [ ] Support streaming output collection during process execution
- [ ] Add output buffer management with configurable limits
- [ ] Handle output encoding and character validation

### Byte Limit and Truncation
- [ ] Implement output byte limit enforcement
- [ ] Add truncation from beginning when limit exceeded
- [ ] Ensure truncation at character boundaries per specification
- [ ] Track and report truncation status accurately

### Exit Status Management
- [ ] Track process exit codes and termination signals
- [ ] Report exit status only when process has completed
- [ ] Handle different process termination scenarios
- [ ] Support both normal exit and signal termination

### Buffer Management
- [ ] Implement circular buffer for output with size limits
- [ ] Support efficient output retrieval without blocking
- [ ] Add memory optimization for long-running processes
- [ ] Handle output buffer overflow and truncation

## Testing Requirements
- [ ] Test real-time output capture during process execution
- [ ] Test output byte limit enforcement and truncation
- [ ] Test character boundary truncation maintains valid UTF-8
- [ ] Test exit status tracking for normal and signal termination
- [ ] Test concurrent output access from multiple requests
- [ ] Test memory management for long-running processes
- [ ] Test error scenarios and proper error responses

## Acceptance Criteria
- Real-time output capture from running processes
- Output byte limit enforcement with beginning truncation
- Character boundary truncation maintaining valid UTF-8 strings
- Exit status reporting with exit codes and signals
- Memory-efficient buffer management for long-running processes
- Non-blocking output retrieval for concurrent operations
- Integration with terminal lifecycle and registry management
- Comprehensive test coverage for all output scenarios