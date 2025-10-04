//! Integration tests for user permission interaction
//!
//! This module tests the complete permission workflow including:
//! - Mock prompt handler returns correct user selections
//! - Permission storage correctly stores "always" decisions and ignores "once" decisions
//! - Allow-always flow: user selection → storage → retrieval for next call
//! - Reject-always flow: user selection → storage → retrieval for next call
//! - Storage operations: clear, remove, overwrite preferences

#[cfg(test)]
mod tests {
    use crate::permission_storage::PermissionStorage;
    use crate::tools::{PermissionOption, PermissionOptionKind};
    use crate::user_prompt::{MockPromptHandler, UserPromptHandler};

    #[tokio::test]
    async fn test_mock_prompt_handler_returns_selected_option() {
        let options = vec![
            PermissionOption {
                option_id: "allow-once".to_string(),
                name: "Allow Once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
            },
            PermissionOption {
                option_id: "allow-always".to_string(),
                name: "Allow Always".to_string(),
                kind: PermissionOptionKind::AllowAlways,
            },
        ];

        let handler = MockPromptHandler::new(Some("allow-always".to_string()));
        let result = handler
            .prompt_for_permission("test_tool", "Test description", &options)
            .await
            .unwrap();

        assert_eq!(result, "allow-always");
    }

    #[tokio::test]
    async fn test_permission_storage_only_stores_always_options() {
        let storage = PermissionStorage::new();

        // Store all four types
        storage
            .store_preference("tool1", PermissionOptionKind::AllowOnce)
            .await;
        storage
            .store_preference("tool2", PermissionOptionKind::AllowAlways)
            .await;
        storage
            .store_preference("tool3", PermissionOptionKind::RejectOnce)
            .await;
        storage
            .store_preference("tool4", PermissionOptionKind::RejectAlways)
            .await;

        // Only "always" types should be stored
        assert_eq!(storage.get_preference("tool1").await, None);
        assert!(matches!(
            storage.get_preference("tool2").await,
            Some(PermissionOptionKind::AllowAlways)
        ));
        assert_eq!(storage.get_preference("tool3").await, None);
        assert!(matches!(
            storage.get_preference("tool4").await,
            Some(PermissionOptionKind::RejectAlways)
        ));

        assert_eq!(storage.count().await, 2);
    }

    #[tokio::test]
    async fn test_permission_storage_retrieves_stored_preference() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("my_tool", PermissionOptionKind::AllowAlways)
            .await;

        let preference = storage.get_preference("my_tool").await;
        assert!(matches!(
            preference,
            Some(PermissionOptionKind::AllowAlways)
        ));
    }

    #[tokio::test]
    async fn test_permission_storage_returns_none_for_nonexistent() {
        let storage = PermissionStorage::new();

        let preference = storage.get_preference("nonexistent_tool").await;
        assert_eq!(preference, None);
    }

    #[tokio::test]
    async fn test_permission_storage_can_be_cleared() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("tool1", PermissionOptionKind::AllowAlways)
            .await;
        storage
            .store_preference("tool2", PermissionOptionKind::RejectAlways)
            .await;

        assert_eq!(storage.count().await, 2);

        storage.clear_all().await;

        assert_eq!(storage.count().await, 0);
        assert_eq!(storage.get_preference("tool1").await, None);
        assert_eq!(storage.get_preference("tool2").await, None);
    }

    #[tokio::test]
    async fn test_permission_storage_removes_specific_preference() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("tool1", PermissionOptionKind::AllowAlways)
            .await;
        storage
            .store_preference("tool2", PermissionOptionKind::RejectAlways)
            .await;

        let removed = storage.remove_preference("tool1").await;
        assert!(removed);

        assert_eq!(storage.count().await, 1);
        assert_eq!(storage.get_preference("tool1").await, None);
        assert!(storage.get_preference("tool2").await.is_some());
    }

    #[tokio::test]
    async fn test_permission_storage_overwrites_existing_preference() {
        let storage = PermissionStorage::new();

        storage
            .store_preference("tool1", PermissionOptionKind::AllowAlways)
            .await;

        let pref = storage.get_preference("tool1").await;
        assert!(matches!(pref, Some(PermissionOptionKind::AllowAlways)));

        // Overwrite with different preference
        storage
            .store_preference("tool1", PermissionOptionKind::RejectAlways)
            .await;

        let pref = storage.get_preference("tool1").await;
        assert!(matches!(pref, Some(PermissionOptionKind::RejectAlways)));

        // Should still have exactly one entry
        assert_eq!(storage.count().await, 1);
    }

    #[tokio::test]
    async fn test_allow_always_stores_preference_and_auto_allows_next_call() {
        let storage = PermissionStorage::new();
        let handler = MockPromptHandler::new(Some("allow-always".to_string()));

        let options = vec![
            PermissionOption {
                option_id: "allow-once".to_string(),
                name: "Allow Once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
            },
            PermissionOption {
                option_id: "allow-always".to_string(),
                name: "Allow Always".to_string(),
                kind: PermissionOptionKind::AllowAlways,
            },
            PermissionOption {
                option_id: "reject-once".to_string(),
                name: "Reject Once".to_string(),
                kind: PermissionOptionKind::RejectOnce,
            },
        ];

        // First call: user selects "allow-always"
        let selected = handler
            .prompt_for_permission("test_tool", "First call", &options)
            .await
            .unwrap();
        assert_eq!(selected, "allow-always");

        // Find the selected option and store its kind
        let selected_option = options
            .iter()
            .find(|opt| opt.option_id == selected)
            .unwrap();
        storage
            .store_preference("test_tool", selected_option.kind.clone())
            .await;

        // Verify the preference was stored
        let stored = storage.get_preference("test_tool").await;
        assert!(matches!(stored, Some(PermissionOptionKind::AllowAlways)));

        // Second call: should retrieve stored preference
        let stored_pref = storage.get_preference("test_tool").await;
        assert!(stored_pref.is_some());
        assert!(matches!(
            stored_pref.unwrap(),
            PermissionOptionKind::AllowAlways
        ));
    }

    #[tokio::test]
    async fn test_reject_always_stores_preference_and_auto_rejects_next_call() {
        let storage = PermissionStorage::new();
        let handler = MockPromptHandler::new(Some("reject-always".to_string()));

        let options = vec![
            PermissionOption {
                option_id: "allow-once".to_string(),
                name: "Allow Once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
            },
            PermissionOption {
                option_id: "reject-once".to_string(),
                name: "Reject Once".to_string(),
                kind: PermissionOptionKind::RejectOnce,
            },
            PermissionOption {
                option_id: "reject-always".to_string(),
                name: "Reject Always".to_string(),
                kind: PermissionOptionKind::RejectAlways,
            },
        ];

        // First call: user selects "reject-always"
        let selected = handler
            .prompt_for_permission("dangerous_tool", "First call", &options)
            .await
            .unwrap();
        assert_eq!(selected, "reject-always");

        // Find the selected option and store its kind
        let selected_option = options
            .iter()
            .find(|opt| opt.option_id == selected)
            .unwrap();
        storage
            .store_preference("dangerous_tool", selected_option.kind.clone())
            .await;

        // Verify the preference was stored
        let stored = storage.get_preference("dangerous_tool").await;
        assert!(matches!(stored, Some(PermissionOptionKind::RejectAlways)));

        // Second call: should retrieve stored preference
        let stored_pref = storage.get_preference("dangerous_tool").await;
        assert!(stored_pref.is_some());
        assert!(matches!(
            stored_pref.unwrap(),
            PermissionOptionKind::RejectAlways
        ));
    }
}
