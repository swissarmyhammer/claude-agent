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