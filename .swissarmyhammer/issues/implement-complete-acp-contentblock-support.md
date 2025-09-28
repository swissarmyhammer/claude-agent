# Implement Complete ACP ContentBlock Type Support

## Problem
Our content block implementation may not fully support all 5 required content block types specified in the ACP specification. While we use `agent-client-protocol` types, we need to ensure complete handling of all content variants including complex nested structures.

## ACP Specification Requirements
From https://agentclientprotocol.com/protocol/content:

**Required ContentBlock Types:**
1. **Text Content** (MUST support)
2. **Image Content** (requires `image` capability)
3. **Audio Content** (requires `audio` capability)
4. **Embedded Resource** (requires `embeddedContext` capability)
5. **Resource Link** (MUST support)

## Content Block Implementations Needed

### Text Content (Basic - MUST support)
```json
{
  "type": "text",
  "text": "What's the weather like today?",
  "annotations": {...}
}
```

### Image Content  
```json
{
  "type": "image",
  "mimeType": "image/png",
  "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB...",
  "uri": "optional-source-uri",
  "annotations": {...}
}
```

### Audio Content
```json
{
  "type": "audio", 
  "mimeType": "audio/wav",
  "data": "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAAB...",
  "annotations": {...}
}
```

### Embedded Resource (Complex nested structure)
```json
{
  "type": "resource",
  "resource": {
    "uri": "file:///home/user/script.py",
    "text": "def hello():\n print('Hello, world!')",  // OR blob for binary
    "mimeType": "text/x-python"
  },
  "annotations": {...}
}
```

### Resource Link
```json
{
  "type": "resource_link",
  "uri": "file:///home/user/document.pdf", 
  "name": "document.pdf",
  "mimeType": "application/pdf",
  "title": "Project Documentation",
  "description": "Complete project documentation",
  "size": 1024000,
  "annotations": {...}
}
```

## Implementation Tasks

### ContentBlock Handler Infrastructure
- [ ] Create comprehensive content block processing system
- [ ] Implement handlers for each content type
- [ ] Add content type detection and routing
- [ ] Support proper serialization/deserialization for all types

### Text Content Support
- [ ] Implement text content processing (likely already exists)
- [ ] Add text length validation and limits
- [ ] Support text annotations processing
- [ ] Handle text encoding edge cases

### Image Content Support  
- [ ] Implement base64 image data handling
- [ ] Add image MIME type validation (`image/png`, `image/jpeg`, etc.)
- [ ] Support optional URI field for image sources
- [ ] Add image size and format validation
- [ ] Implement image content security checks

### Audio Content Support
- [ ] Implement base64 audio data handling
- [ ] Add audio MIME type validation (`audio/wav`, `audio/mp3`, etc.)
- [ ] Support audio content processing and validation
- [ ] Add audio duration and size limits
- [ ] Implement audio content security checks

### Embedded Resource Support
- [ ] Handle complex nested resource structure
- [ ] Support both text and blob resource variants (mutually exclusive)
- [ ] Implement URI processing for embedded resources
- [ ] Add MIME type validation for embedded resources
- [ ] Handle text vs binary content detection

### Resource Link Support
- [ ] Implement URI-based resource references
- [ ] Add resource metadata processing (name, title, description, size)
- [ ] Support resource link validation
- [ ] Handle resource accessibility checking
- [ ] Add resource size and type validation

## Content Type Detection
```rust
pub fn process_content_block(content: &ContentBlock) -> Result<ProcessedContent> {
    match content {
        ContentBlock::Text(text) => process_text_content(text),
        ContentBlock::Image(image) => process_image_content(image), 
        ContentBlock::Audio(audio) => process_audio_content(audio),
        ContentBlock::Resource(resource) => process_embedded_resource(resource),
        ContentBlock::ResourceLink(link) => process_resource_link(link),
    }
}
```

## Implementation Notes
Add content block processing comments:
```rust
// ACP requires support for all 5 ContentBlock types:
// 1. Text: Always supported (mandatory)
// 2. Image: Base64 data + MIME type validation
// 3. Audio: Base64 data + MIME type validation  
// 4. Resource: Complex nested structure with text/blob variants
// 5. ResourceLink: URI-based resource references with metadata
//
// Content must be validated against declared prompt capabilities.
```

### MCP Compatibility
- [ ] Ensure perfect compatibility with MCP ContentBlock structure
- [ ] Support seamless forwarding of MCP tool outputs
- [ ] Maintain same field names and serialization format
- [ ] Handle MCP-specific edge cases

### Error Handling
- [ ] Handle unsupported content types gracefully
- [ ] Validate content structure and required fields
- [ ] Provide clear error messages for malformed content
- [ ] Handle edge cases in content processing

## Testing Requirements
- [ ] Test all 5 content block types processing
- [ ] Test content type detection and routing
- [ ] Test base64 data handling for images and audio
- [ ] Test embedded resource text vs blob variants
- [ ] Test resource link metadata processing
- [ ] Test content validation and error scenarios
- [ ] Test MCP compatibility and forwarding
- [ ] Test content size limits and security validation

## Integration Points
- [ ] Connect to content capability validation system
- [ ] Integrate with prompt processing system
- [ ] Connect to tool result content formatting
- [ ] Integrate with session update content streaming

## Performance Considerations
- [ ] Optimize base64 encoding/decoding for large content
- [ ] Handle large embedded resources efficiently
- [ ] Support streaming for large content blocks
- [ ] Add memory usage limits for content processing

## Acceptance Criteria
- Complete support for all 5 ACP ContentBlock types
- Proper base64 data handling for images and audio
- Complex embedded resource structure handling
- Resource link metadata processing
- MCP-compatible content block forwarding
- Comprehensive content validation and error handling
- Performance optimization for large content
- Complete test coverage for all content types
- Integration with existing content processing systems