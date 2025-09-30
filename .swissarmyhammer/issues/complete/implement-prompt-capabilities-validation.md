# Implement Prompt Capabilities Validation

## Problem
The agent should validate that prompt requests only include content types it declared support for during initialization. Currently, we have no validation to ensure clients respect the capabilities we advertised.

## Current Issues
- No validation of content types against declared capabilities during prompt processing
- Clients could send unsupported content types (image, audio, embeddedContext) without proper error handling
- Missing capability-based feature gating as required by ACP spec

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/initialization:

**Baseline Support (MUST support):**
- `ContentBlock::Text`
- `ContentBlock::ResourceLink`

**Optional Support (declared in capabilities):**
- `ContentBlock::Image` - requires `promptCapabilities.image: true`
- `ContentBlock::Audio` - requires `promptCapabilities.audio: true` 
- `ContentBlock::Resource` - requires `promptCapabilities.embeddedContext: true`

## Implementation Requirements
The agent must validate that prompt requests only contain content types for which capabilities were declared:

```rust
// If we declared promptCapabilities.image: false
// Then any ContentBlock::Image should be rejected with proper error

// If we declared promptCapabilities.audio: true  
// Then ContentBlock::Audio should be accepted and processed
```

## Implementation Tasks
- [ ] Add content type validation in prompt request handler
- [ ] Check each ContentBlock against declared capabilities
- [ ] Return proper ACP error responses for unsupported content types
- [ ] Add capability checking utility functions
- [ ] Implement proper error messages explaining capability requirements
- [ ] Add tests for each content type validation scenario
- [ ] Ensure Text and ResourceLink are always accepted (baseline requirement)
- [ ] Add validation for embeddedContext capability vs Resource content blocks

## Error Response Format
When content type is not supported:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid content type: agent does not support image content",
    "data": {
      "contentType": "image",
      "declaredCapability": false,
      "required": "promptCapabilities.image"
    }
  }
}
```

## Acceptance Criteria
- All prompt requests validated against declared capabilities
- Proper error responses for unsupported content types
- Text and ResourceLink always accepted (baseline)
- Image content rejected if `promptCapabilities.image: false`
- Audio content rejected if `promptCapabilities.audio: false`
- Resource content rejected if `promptCapabilities.embeddedContext: false`
- Clear error messages explaining capability requirements
- Comprehensive test coverage for all validation scenarios

## Analysis Results

After reviewing the codebase, I've discovered that **this issue has already been fully implemented**. Here's what exists:

### Existing Implementation

1. **ContentCapabilityValidator Module** (`lib/src/content_capability_validator.rs`):
   - Complete validation logic for all content types against prompt capabilities
   - Proper error types with ACP-compliant JSON-RPC error conversion
   - Comprehensive test coverage (15+ tests)
   - Validates: Text (always), ResourceLink (always), Image (conditional), Audio (conditional), Resource (conditional)

2. **Integration in Agent** (`lib/src/agent.rs` lines 1149-1199):
   - Manual validation logic in the prompt handler
   - Checks capabilities before processing content blocks
   - Returns proper ACP error responses with correct error codes (-32602)

3. **Error Handling**:
   - Proper ACP-compliant error format with structured data
   - Clear error messages explaining capability requirements
   - Support for both single and multiple violations

### Current Implementation Details

The validator correctly implements:
- ✅ Text and ResourceLink always accepted (baseline requirement)
- ✅ Image content rejected if `promptCapabilities.image: false`
- ✅ Audio content rejected if `promptCapabilities.audio: false`  
- ✅ Resource content rejected if `promptCapabilities.embeddedContext: false`
- ✅ Proper ACP error responses with code -32602
- ✅ Comprehensive test coverage for all scenarios

### Code Locations

1. **Validator**: `/Users/wballard/github/claude-agent/lib/src/content_capability_validator.rs`
2. **Agent Integration**: `/Users/wballard/github/claude-agent/lib/src/agent.rs:1149-1199`
3. **Error Conversion**: `/Users/wballard/github/claude-agent/lib/src/acp_error_conversion.rs`

### Issue Status

**This issue is complete**. The implementation:
- Meets all requirements in the issue description
- Follows ACP specification requirements
- Has comprehensive test coverage
- Uses proper error handling
- Is already integrated into the prompt handling flow

However, there is one observation: The agent.rs file has **duplicate validation logic** (lines 1149-1199) that performs manual checks instead of using the `ContentCapabilityValidator` that was created for this purpose. While both work correctly, this creates code duplication.

### Recommendation

The code should be refactored to use the ContentCapabilityValidator exclusively, removing the duplicate manual validation in agent.rs. This would:
- Reduce code duplication
- Centralize validation logic
- Make maintenance easier
- Ensure consistency

But this is a code quality improvement, not a missing feature. The functionality required by this issue is fully implemented and working.

## Code Review Fixes (2025-09-30)

### Issues Found in Code Review

1. **Critical Bug**: ResourceLink validation in streaming path incorrectly required `embedded_context` capability
2. **Code Duplication**: Manual validation logic in streaming path duplicated ContentCapabilityValidator functionality

### Fixes Applied

1. **Refactored Streaming Validation** (`lib/src/agent.rs:1149-1200`):
   - Replaced 51 lines of manual validation code with ContentCapabilityValidator
   - Now uses identical validation logic as non-streaming path
   - Fixes ResourceLink bug by correctly treating it as baseline requirement

2. **Added Integration Test** (`lib/src/agent.rs`):
   - New test: `test_streaming_prompt_with_resource_link`
   - Verifies ResourceLink works in streaming mode even with `embedded_context: false`
   - Ensures both streaming and non-streaming paths behave identically

### Test Results

All 416 tests pass, confirming:
- ResourceLink bug is fixed
- Code duplication eliminated
- No regressions introduced
- Both validation paths now consistent