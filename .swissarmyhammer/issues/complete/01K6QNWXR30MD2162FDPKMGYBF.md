# Implement Session Loading Capability Support

## Description
Session loading functionality has incomplete capability support implementations.

## Found Issues
- `session_loading.rs:1`: Contains TODO/placeholder for loadSession capability support
- Missing proper capability validation and support checking

## Priority
Medium - Session management functionality

## Files Affected
- `lib/src/session_loading.rs`
## Proposed Solution

After thorough investigation, I found that the issue description is **outdated**. The TODO mentioned in the issue has already been resolved in commit `c49a1bb` which implemented "MCP server validation in session loading".

### Current State Analysis

1. **Capability Validation is Complete**: The `load_session_enhanced` method in `session_loading.rs:60-68` properly validates the `loadSession` capability:
   ```rust
   if !capabilities_load_session {
       warn!("Session load requested but loadSession capability not supported");
       return Err(SessionSetupError::LoadSessionNotSupported {
           declared_capability: false,
       });
   }
   ```

2. **Tests Pass**: All 14 session_loading tests and 40 capability tests pass successfully.

3. **Complete Implementation Chain**:
   - `agent.rs:2448-2451` - Uses `CapabilityRequirementChecker::check_load_session_requirements()` to validate transport and loadSession capability
   - `agent.rs:2457-2469` - Has a second redundant check for `load_session` capability
   - `capability_validation.rs:354-371` - Implements the comprehensive capability check
   - `session_loading.rs:60-68` - Implements capability validation within the session loader

### Issues Found

**Code Duplication**: The capability validation happens in THREE places:
1. `agent.rs:2448` - Via `CapabilityRequirementChecker`
2. `agent.rs:2457` - Direct check with duplicate error creation
3. `session_loading.rs:66` - Inside the session loader

This violates the DRY principle and creates maintenance burden.

### Recommended Solution

**Consolidate capability validation to a single location** to eliminate duplication:

1. Remove the redundant check in `agent.rs:2457-2469` since it's already done by `CapabilityRequirementChecker` at line 2448
2. Keep the validation in `session_loading.rs` as the authoritative check since it's closest to where the capability is actually used
3. The `agent.rs` level should only do the transport capability validation via `CapabilityRequirementChecker`

This creates a clean separation:
- `agent.rs` - Validates transport capabilities for MCP servers
- `session_loading.rs` - Validates loadSession capability as part of session loading logic

## Implementation Complete

### Changes Made

**Eliminated duplicate capability validation in `agent.rs:2456-2469`**

Removed the redundant `loadSession` capability check that was occurring after the comprehensive validation via `CapabilityRequirementChecker`. This check was unnecessary because:

1. `CapabilityRequirementChecker::check_load_session_requirements()` at line 2448 already validates the `loadSession` capability
2. The error handling was duplicated - both checks returned the same error type
3. This violated the DRY principle and created maintenance burden

### Validation Flow After Changes

The capability validation now follows a clean, single-path approach:

1. **Transport + LoadSession Capability Check** (`agent.rs:2448`)
   - `CapabilityRequirementChecker::check_load_session_requirements()` validates:
     - `loadSession` capability is enabled
     - Required MCP transport capabilities match requested servers
   - Returns `SessionSetupError` on failure, converted to ACP error

2. **Session Loading Logic** (`session_loading.rs:66`)
   - `load_session_enhanced()` receives capability as parameter
   - Validates capability at the point of use
   - This provides defense-in-depth without duplication

### Test Results

✅ **All tests pass**: 631 tests run: 631 passed
✅ **Cargo build**: Success
✅ **Cargo fmt**: Clean
✅ **Cargo clippy**: No warnings

### Files Modified

- `lib/src/agent.rs:2456-2469` - Removed duplicate capability check
