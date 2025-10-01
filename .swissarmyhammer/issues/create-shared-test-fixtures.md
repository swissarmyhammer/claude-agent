# Create Shared Test Fixtures Module

## Problem
Test setup code is heavily duplicated across 30+ test modules:
- 15+ duplicate helper functions
- Repeated permission engine and session manager creation
- Duplicated content block creation helpers
- Hardcoded test data (base64 strings) repeated in multiple files

## Duplicated Patterns

### Permission Engine Setup
Appears in:
- `tool_call_lifecycle_tests.rs`
- `tools.rs`
```rust
fn create_test_permission_engine() -> std::sync::Arc<crate::permissions::PermissionPolicyEngine> {
    use crate::permissions::{FilePermissionStorage, PermissionPolicyEngine};
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    std::sync::Arc::new(PermissionPolicyEngine::new(Box::new(storage)))
}
```

### Tool Handler Creation
Appears in:
- `tool_call_lifecycle_tests.rs`
- `tools.rs`
```rust
async fn create_test_handler() -> (ToolCallHandler, broadcast::Receiver<SessionNotification>) {
    let permissions = ToolPermissions { ... };
    let session_manager = std::sync::Arc::new(crate::session::SessionManager::new());
    let permission_engine = create_test_permission_engine();
    // ... 10 more lines
}
```

### Content Block Creation
Appears in:
- `content_capability_validator.rs`
- `content_security_integration_tests.rs`
- `content_block_processor.rs`
```rust
fn create_test_text_content() -> ContentBlock { ... }
fn create_test_resource_link_content() -> ContentBlock { ... }
fn create_test_image_content() -> ContentBlock { ... }
fn create_test_audio_content() -> ContentBlock { ... }
```

### Test Data Constants
Repeated in multiple files:
```rust
let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
let wav_data = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";
```

## Recommendation

### Create Test Fixtures Module
**New file:** `lib/tests/common/fixtures.rs`

```rust
/// Common test fixtures and utilities
pub mod fixtures {
    use std::sync::Arc;
    use tempfile::TempDir;
    
    /// Create test permission engine with temp storage
    pub fn permission_engine() -> Arc<crate::permissions::PermissionPolicyEngine> {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = crate::permissions::FilePermissionStorage::new(
            temp_dir.path().to_path_buf()
        );
        Arc::new(crate::permissions::PermissionPolicyEngine::new(Box::new(storage)))
    }
    
    /// Create test session manager
    pub fn session_manager() -> Arc<crate::session::SessionManager> {
        Arc::new(crate::session::SessionManager::new())
    }
    
    /// Create temp directory for testing
    pub fn temp_storage() -> (TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        (dir, path)
    }
    
    /// Create default test tool permissions
    pub fn tool_permissions() -> crate::tools::ToolPermissions {
        crate::tools::ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["test_tool".to_string()],
            forbidden_paths: vec![],
        }
    }
    
    pub mod content_blocks {
        /// Create text content block
        pub fn text(content: &str) -> ContentBlock {
            ContentBlock::Text {
                text: content.to_string(),
            }
        }
        
        /// Create image content block
        pub fn image(mime_type: &str, data: &str) -> ContentBlock {
            ContentBlock::Image {
                mime_type: mime_type.to_string(),
                data: data.to_string(),
            }
        }
        
        /// Create audio content block
        pub fn audio(mime_type: &str, data: &str) -> ContentBlock {
            ContentBlock::Audio {
                mime_type: mime_type.to_string(),
                data: data.to_string(),
            }
        }
    }
    
    pub mod test_data {
        /// Valid 1x1 PNG image (base64)
        pub const VALID_PNG_BASE64: &str = 
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        
        /// Valid WAV audio (base64)
        pub const VALID_WAV_BASE64: &str = 
            "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";
        
        /// Malicious PE file header (for security tests)
        pub const MALICIOUS_PE_BASE64: &str = "TVqQAAMAAAAEAAAA//8AALgA";
    }
}
```

**New file:** `lib/tests/common/handler_utils.rs`

```rust
/// Test utilities for ToolCallHandler
pub mod handler_utils {
    use tokio::sync::broadcast;
    
    /// Create test handler with notification receiver
    pub async fn create_test_handler_with_notifications() 
        -> (crate::tools::ToolCallHandler, broadcast::Receiver<crate::session::SessionNotification>) 
    {
        let permissions = fixtures::tool_permissions();
        let session_manager = fixtures::session_manager();
        let permission_engine = fixtures::permission_engine();
        
        let mut handler = crate::tools::ToolCallHandler::new(
            permissions, 
            session_manager, 
            permission_engine
        );
        
        let (sender, receiver) = crate::session::NotificationSender::new(32);
        handler.set_notification_sender(sender);
        
        (handler, receiver)
    }
    
    /// Consume and return next notification
    pub async fn consume_notification(
        receiver: &mut broadcast::Receiver<crate::session::SessionNotification>
    ) -> crate::session::SessionNotification {
        receiver.recv().await.expect("Should receive notification")
    }
}
```

**New file:** `lib/tests/common/mod.rs`

```rust
pub mod fixtures;
pub mod handler_utils;
pub mod security_helpers;
pub mod assertions;
```

### Update Test Modules
Replace duplicated code with imports:
```rust
use tests::common::fixtures;
use tests::common::test_data;

#[tokio::test]
async fn test_something() {
    let permission_engine = fixtures::permission_engine();
    let image = fixtures::content_blocks::image("image/png", test_data::VALID_PNG_BASE64);
    // ...
}
```

## Impact
- Reduces test boilerplate by 30-40%
- Eliminates ~150+ lines of duplicated code
- Consistent test setup across all modules
- Easier to maintain and update test helpers
- Single source of truth for test data