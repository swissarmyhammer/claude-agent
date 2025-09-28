# Implement Content Block Validation Against Prompt Capabilities

## Problem
Our content processing doesn't validate content blocks against declared prompt capabilities as required by the ACP specification. We need to ensure only supported content types are accepted based on the capabilities negotiated during initialization.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/content and https://agentclientprotocol.com/protocol/initialization:

**Content Type Capability Rules:**
- **Text Content**: Always allowed (MUST support)
- **Resource Link**: Always allowed (MUST support)  
- **Image Content**: Only if `promptCapabilities.image: true`
- **Audio Content**: Only if `promptCapabilities.audio: true`
- **Embedded Resource**: Only if `promptCapabilities.embeddedContext: true`

**Capability Declaration Example:**
```json
{
  "agentCapabilities": {
    "promptCapabilities": {
      "image": true,
      "audio": false,
      "embeddedContext": true
    }
  }
}
```

## Current Issues
- Content blocks may be accepted regardless of declared capabilities
- Missing validation against `promptCapabilities` during prompt processing
- No proper error responses for capability violations
- Missing capability checking for different content contexts

## Implementation Tasks

### Capability Storage and Access
- [ ] Store prompt capabilities from initialization response
- [ ] Make capabilities accessible during content validation
- [ ] Add capability lookup utilities for content validation
- [ ] Ensure capabilities persist throughout session lifecycle

### Content Validation Logic
- [ ] Always allow text content blocks (baseline requirement)
- [ ] Always allow resource link content blocks (baseline requirement)
- [ ] Validate image content only if `promptCapabilities.image: true`
- [ ] Validate audio content only if `promptCapabilities.audio: true`
- [ ] Validate embedded resources only if `promptCapabilities.embeddedContext: true`

### Validation Integration Points
- [ ] Add content validation to `session/prompt` processing
- [ ] Validate content in tool call results against capabilities
- [ ] Add validation to session update content streaming
- [ ] Ensure validation occurs before content processing

### Error Response Implementation
- [ ] Return proper ACP errors for unsupported content types
- [ ] Include capability information in error responses
- [ ] Provide clear error messages explaining requirements
- [ ] Add structured error data for programmatic handling

## Validation Implementation
```rust
pub struct ContentValidator {
    prompt_capabilities: PromptCapabilities,
}

impl ContentValidator {
    pub fn validate_content_block(&self, content: &ContentBlock) -> Result<(), ValidationError> {
        match content {
            ContentBlock::Text(_) => Ok(()), // Always allowed
            ContentBlock::ResourceLink(_) => Ok(()), // Always allowed
            ContentBlock::Image(_) => {
                if self.prompt_capabilities.image {
                    Ok(())
                } else {
                    Err(ValidationError::UnsupportedContent("image"))
                }
            }
            ContentBlock::Audio(_) => {
                if self.prompt_capabilities.audio {
                    Ok(())
                } else {
                    Err(ValidationError::UnsupportedContent("audio"))
                }
            }
            ContentBlock::Resource(_) => {
                if self.prompt_capabilities.embedded_context {
                    Ok(())
                } else {
                    Err(ValidationError::UnsupportedContent("embeddedContext"))
                }
            }
        }
    }
}
```

## Error Response Examples
For unsupported image content:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid content type: agent does not support image content",
    "data": {
      "contentType": "image",
      "declaredCapability": false,
      "required": "promptCapabilities.image",
      "supportedTypes": ["text", "resource_link"]
    }
  }
}
```

For unsupported audio content:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid content type: agent does not support audio content",
    "data": {
      "contentType": "audio", 
      "declaredCapability": false,
      "required": "promptCapabilities.audio",
      "supportedTypes": ["text", "resource_link", "image"]
    }
  }
}
```

For unsupported embedded resources:
```json
{
  "error": {
    "code": -32602,
    "message": "Invalid content type: agent does not support embedded resources",
    "data": {
      "contentType": "resource",
      "declaredCapability": false,
      "required": "promptCapabilities.embeddedContext",
      "supportedTypes": ["text", "resource_link"]
    }
  }
}
```

## Implementation Notes
Add content validation comments:
```rust
// ACP requires strict content validation against declared capabilities:
// 1. Text and ResourceLink: Always supported (baseline)
// 2. Image: Only if promptCapabilities.image: true
// 3. Audio: Only if promptCapabilities.audio: true  
// 4. Resource: Only if promptCapabilities.embeddedContext: true
//
// This prevents protocol violations and ensures capability contract compliance.
```

### Validation Contexts
- [ ] Validate content in user prompts (`session/prompt`)
- [ ] Validate content in tool call results
- [ ] Validate content in session update notifications
- [ ] Handle different validation rules for different contexts

### Batch Content Validation
- [ ] Validate arrays of content blocks efficiently
- [ ] Handle mixed content types in single request
- [ ] Provide detailed validation results for multiple content blocks
- [ ] Support partial validation success/failure scenarios

### Capability Configuration
- [ ] Support runtime capability updates (if allowed by spec)
- [ ] Handle capability negotiation edge cases
- [ ] Add logging for capability validation decisions
- [ ] Support debugging of capability validation logic

## Testing Requirements
- [ ] Test baseline content types always allowed (text, resource_link)
- [ ] Test image content rejected when `promptCapabilities.image: false`
- [ ] Test audio content rejected when `promptCapabilities.audio: false`
- [ ] Test embedded resources rejected when `promptCapabilities.embeddedContext: false`
- [ ] Test mixed content arrays with partial capability support
- [ ] Test proper error responses for all unsupported content types
- [ ] Test validation in different contexts (prompts, tool results, etc.)
- [ ] Test capability validation performance with large content arrays

## Integration Points
- [ ] Connect to initialization capability storage
- [ ] Integrate with prompt processing system
- [ ] Connect to tool call result processing
- [ ] Integrate with session update content handling

## Performance Considerations
- [ ] Optimize validation for large content block arrays
- [ ] Cache capability lookups for repeated validation
- [ ] Support fast-path validation for common content types
- [ ] Minimize validation overhead in content processing pipeline

## Acceptance Criteria
- All content blocks validated against declared prompt capabilities
- Text and resource_link content always allowed (baseline requirement)
- Image content only allowed if `promptCapabilities.image: true`
- Audio content only allowed if `promptCapabilities.audio: true`
- Embedded resources only allowed if `promptCapabilities.embeddedContext: true`
- Proper ACP error responses for capability violations
- Clear error messages explaining capability requirements
- Validation integrated into all content processing contexts
- Complete test coverage for all capability/content combinations
- Performance optimization for content validation pipeline