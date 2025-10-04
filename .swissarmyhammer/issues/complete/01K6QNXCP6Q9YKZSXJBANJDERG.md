# Implement Terminal Capability Checking and Permission Conversion

## Description
Tool system has placeholders for terminal capabilities and permission handling.

## Found Issues
- `tools.rs:1`: Terminal capability checking is placeholder
- `tools.rs:1`: Permission conversion needs proper implementation
- Missing proper tool execution validation

## Priority
Medium - Tool execution system

## Files Affected
- `lib/src/tools.rs`


## Proposed Solution

After analyzing the code, I've identified the following issues and solutions:

### Issue 1: Permission Conversion (Line 857)
**Problem**: The code creates an `EnhancedPermissionRequest` with session_id and options, but then discards these fields when converting to the simple `PermissionRequest` structure. The comment says "will be enhanced later" but this loses important information.

**Analysis**: 
- `EnhancedPermissionRequest` has: session_id, tool_request_id, tool_name, description, arguments, options
- Simple `PermissionRequest` has: tool_request_id, tool_name, description, arguments
- The options field is critical for ACP compliance - it tells the user what choices they have
- The session_id is needed to track which session this permission belongs to

**Solution**: 
The simple `PermissionRequest` in tools.rs (line 222) should be enhanced to match the structure needed. However, looking at the usage, the current simple structure is what's used in ToolCallResult. The proper fix is to:
1. Remove the duplicate `PermissionRequest` structure in tools.rs
2. Use the ACP-compliant `PermissionRequest` from agent.rs instead
3. Update the conversion to properly map EnhancedPermissionRequest to the ACP-compliant structure
4. This requires converting tool_request_id/tool_name/arguments into a ToolCallUpdate structure

### Issue 2: Terminal Capability Checking (Line 789)
**Problem**: The issue description says "Terminal capability checking is placeholder" but the implementation at line 789 looks complete.

**Analysis**:
- The `validate_terminal_capability` method checks if client_capabilities.terminal is true
- It returns proper errors if not supported or if capabilities are missing
- It's called before terminal operations (lines 1408, 1426)
- Tests verify the behavior (lines 2796+, 2906+, 3315+)

**Solution**: 
The terminal capability checking is actually properly implemented. This is NOT a placeholder. The issue description may be outdated or incorrect.

### Issue 3: Tool Execution Validation
**Problem**: The issue mentions "Missing proper tool execution validation"

**Analysis**:
Looking at the code flow in `handle_tool_request`:
1. Creates tool call report
2. Evaluates policy (PermissionRequired/Denied/Allowed)
3. Updates status to in_progress
4. Executes tool request
5. Completes tool call report

The validation that exists:
- Policy evaluation via permission_engine
- Terminal capability checking for terminal tools
- Path validation in tests

**Solution**:
Need to identify what specific validation is missing. Candidates:
- Validate arguments match tool schema
- Validate session exists before executing
- Validate tool name is recognized
- These validations may exist in execute_tool_request - need to verify

### Implementation Plan
1. Keep the simple PermissionRequest as-is since changing it would require refactoring all usages
2. Add proper conversion from EnhancedPermissionRequest to PermissionRequest (add options field)
3. Verify terminal capability checking is working (write test if needed)
4. Identify and add any missing tool execution validation
5. Run all tests to verify




## Implementation Completed

### Changes Made

1. **Fixed Permission Conversion (Line 857-867)**
   - Added `options` field to `PermissionRequest` struct at line 222
   - Updated permission conversion to preserve the `options` from `EnhancedPermissionRequest`
   - Removed the "will be enhanced later" comment since it's now properly implemented
   - Updated test helper `create_permission_request` to generate proper permission options

2. **Verified Terminal Capability Checking**
   - The terminal capability checking at line 789 is NOT a placeholder
   - It properly validates client capabilities before terminal operations
   - Returns appropriate errors when capabilities are missing
   - Has comprehensive test coverage (lines 2796+, 2906+, 3315+)
   - This was incorrectly identified in the issue description

3. **Verified Tool Execution Validation**
   - The tool execution has proper validation:
     - Policy evaluation via permission_engine
     - Capability validation for fs operations (validate_fs_read_capability, validate_fs_write_capability)
     - Terminal capability validation (validate_terminal_capability)
     - Path security validation (validate_file_path with PathValidator)
     - Session validation before executing tools
   - No additional validation was needed

### Root Cause Analysis

The issue was that the `PermissionRequest` struct was missing the `options` field that comes from the policy engine's `PolicyEvaluation::RequireUserConsent`. This meant that when a tool required user consent, the available permission options (allow once, allow always, reject once, reject always) were being discarded instead of being passed to the user.

The fix ensures that permission options are properly preserved throughout the permission request flow, maintaining ACP compliance.

### Test Results

- Build: ✓ Successful
- Tests: ✓ All 638 tests passed




## Code Review Completed

### Changes Made
Added comprehensive test coverage for the `options` field in `test_create_permission_request` (lib/src/tools.rs:2171).

### Test Validations Added
1. Validates `options` field is not empty
2. Validates exactly 4 permission options are generated
3. Validates all 4 option kinds are present (AllowOnce, AllowAlways, RejectOnce, RejectAlways)
4. Validates moderate risk tools include "use with caution" warning in AllowAlways option

### Test Results
- Specific test: ✓ PASSED
- Full test suite: ✓ ALL 638 TESTS PASSED

### Decision Notes
- Followed the code review recommendation to add missing test coverage
- Test validates both the structure and content of generated options
- Risk level validation ensures the permission system correctly warns users about potentially dangerous operations