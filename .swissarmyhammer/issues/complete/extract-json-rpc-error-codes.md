# Extract JSON-RPC Error Code Constants

## Problem
JSON-RPC error codes are hardcoded throughout the codebase as magic numbers (-32600, -32601, -32602, -32603, -32700, -32000). This makes the code harder to understand and maintain.

## Locations

**Extensive usage in:**
- `agent.rs` - 50+ occurrences
- `acp_error_conversion.rs` - 80+ occurrences  
- `session_errors.rs` - 10+ occurrences
- `error.rs` - 20+ occurrences

**Examples:**
```rust
// agent.rs:566
code: -32600, // Invalid Request - Protocol version mismatch

// agent.rs:655
code: -32602, // Invalid params

// acp_error_conversion.rs:108
code: -32602,

// error.rs:119
McpError::ProtocolError(_) => -32600,       // Invalid Request
McpError::SerializationFailed(_) => -32700, // Parse error
McpError::ServerError(_) => -32000,         // Server error
```

## Issues
1. **Magic numbers** make code hard to understand
2. **Inconsistent comments** - some explain the code, some don't
3. **Error-prone** - easy to use wrong code
4. **No single source of truth** for JSON-RPC error codes

## Recommendation

### Create JSON-RPC Constants Module
**New file:** `lib/src/json_rpc_codes.rs`

```rust
//! JSON-RPC 2.0 Error Codes
//! 
//! Standard error codes as defined in JSON-RPC 2.0 specification:
//! https://www.jsonrpc.org/specification#error_object

/// Parse error - Invalid JSON was received by the server
pub const PARSE_ERROR: i32 = -32700;

/// Invalid Request - The JSON sent is not a valid Request object
pub const INVALID_REQUEST: i32 = -32600;

/// Method not found - The method does not exist / is not available
pub const METHOD_NOT_FOUND: i32 = -32601;

/// Invalid params - Invalid method parameter(s)
pub const INVALID_PARAMS: i32 = -32602;

/// Internal error - Internal JSON-RPC error
pub const INTERNAL_ERROR: i32 = -32603;

/// Server error - Reserved for implementation-defined server errors
pub const SERVER_ERROR: i32 = -32000;

/// Check if error code is a standard JSON-RPC error
pub fn is_standard_error(code: i32) -> bool {
    matches!(
        code,
        PARSE_ERROR | INVALID_REQUEST | METHOD_NOT_FOUND | INVALID_PARAMS | INTERNAL_ERROR
    )
}

/// Check if error code is a server error (implementation-defined)
pub fn is_server_error(code: i32) -> bool {
    (-32099..=-32000).contains(&code)
}

/// Get human-readable description of error code
pub fn error_description(code: i32) -> &'static str {
    match code {
        PARSE_ERROR => "Parse error - Invalid JSON",
        INVALID_REQUEST => "Invalid Request - Not a valid Request object",
        METHOD_NOT_FOUND => "Method not found",
        INVALID_PARAMS => "Invalid params - Invalid method parameter(s)",
        INTERNAL_ERROR => "Internal error",
        code if is_server_error(code) => "Server error",
        _ => "Unknown error",
    }
}
```

### Update Usage

```rust
// Old:
code: -32602, // Invalid params

// New:
use crate::json_rpc_codes;
code: json_rpc_codes::INVALID_PARAMS,
```

```rust
// Old:
match code {
    -32600 => { /* handle invalid request */ }
    -32602 => { /* handle invalid params */ }
    _ => { /* handle other */ }
}

// New:
use crate::json_rpc_codes::{INVALID_REQUEST, INVALID_PARAMS};
match code {
    INVALID_REQUEST => { /* handle invalid request */ }
    INVALID_PARAMS => { /* handle invalid params */ }
    _ => { /* handle other */ }
}
```

## Impact
- Eliminates 150+ magic number occurrences
- Self-documenting code with named constants
- Single source of truth for error codes
- Easier to maintain and update
- Helper functions for validation and description
- Compliance with JSON-RPC 2.0 specification


## Proposed Solution

I will implement this refactoring using Test Driven Development:

1. **Create the constants module** (`lib/src/json_rpc_codes.rs`)
   - Define all 6 JSON-RPC error code constants
   - Implement helper functions: `is_standard_error`, `is_server_error`, `error_description`
   - Write comprehensive unit tests for all helper functions

2. **Systematic replacement strategy**
   - Replace magic numbers file by file in order of smallest to largest impact
   - Start with `error.rs` (~20 occurrences) - core error mapping
   - Then `session_errors.rs` (~10 occurrences) - session error handling
   - Then `acp_error_conversion.rs` (~80 occurrences) - ACP error conversions
   - Finally `agent.rs` (~50 occurrences) - agent error responses

3. **Testing approach**
   - Write tests for the constants module first (TDD)
   - Run full test suite after each file update
   - Ensure no behavioral changes, only code clarity improvements

4. **Module registration**
   - Add the new module to `lib/src/lib.rs`
   - Export constants for use across the codebase

This approach ensures we have tests validating the constants module before using it, and we verify no regressions after each file update.