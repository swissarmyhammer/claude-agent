//! Comprehensive tests for tool call status lifecycle and ACP-compliant notifications
//!
//! This module tests the complete tool call status reporting implementation to ensure
//! full ACP compliance with proper notification sequences for all scenarios.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::NotificationSender;
    use crate::tools::{ToolCallHandler, ToolPermissions};
    use crate::tool_types::{ToolCallStatus, ToolKind};
    use agent_client_protocol::{SessionId, SessionUpdate, SessionNotification};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    /// Helper to create a test tool call handler with notification sender
    async fn create_test_handler() -> (ToolCallHandler, broadcast::Receiver<SessionNotification>) {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["test_tool".to_string()],
            forbidden_paths: vec![],
        };
        
        let mut handler = ToolCallHandler::new(permissions);
        let (sender, receiver) = NotificationSender::new(32);
        handler.set_notification_sender(sender);
        
        (handler, receiver)
    }

    #[tokio::test]
    async fn test_complete_tool_call_lifecycle_success() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_123".into());
        let tool_name = "test_tool";
        let arguments = json!({"param": "value"});

        // 1. Create tool call - should send initial ToolCall notification
        let report = handler.create_tool_call_report(&session_id, tool_name, &arguments).await;
        let tool_call_id = report.tool_call_id.clone();

        // Verify initial notification
        let notification = receiver.recv().await.expect("Should receive initial notification");
        match notification.update {
            SessionUpdate::ToolCall(tool_call) => {
                assert_eq!(tool_call.id.0, tool_call_id);
                assert_eq!(tool_call.status, agent_client_protocol::ToolCallStatus::Pending);
                assert_eq!(tool_call.title, "Test tool");
                assert_eq!(tool_call.kind, agent_client_protocol::ToolKind::Other);
            }
            _ => panic!("Expected ToolCall notification"),
        }

        // 2. Update to in_progress - should send ToolCallUpdate notification
        let updated_report = handler.update_tool_call_report(&session_id, &tool_call_id, |report| {
            report.update_status(ToolCallStatus::InProgress);
        }).await.expect("Should update successfully");

        // Verify progress notification
        let notification = receiver.recv().await.expect("Should receive progress notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0, tool_call_id);
                assert_eq!(update.fields.status, Some(agent_client_protocol::ToolCallStatus::InProgress));
            }
            _ => panic!("Expected ToolCallUpdate notification"),
        }

        // 3. Complete tool call - should send final ToolCallUpdate notification
        let output = json!({"result": "success"});
        let completed_report = handler.complete_tool_call_report(&session_id, &tool_call_id, Some(output.clone())).await
            .expect("Should complete successfully");

        // Verify completion notification
        let notification = receiver.recv().await.expect("Should receive completion notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0, tool_call_id);
                assert_eq!(update.fields.status, Some(agent_client_protocol::ToolCallStatus::Completed));
                assert_eq!(update.fields.raw_output, Some(output));
            }
            _ => panic!("Expected ToolCallUpdate completion notification"),
        }

        // Verify tool call was removed from active tracking
        let active_report = handler.update_tool_call_report(&session_id, &tool_call_id, |_| {}).await;
        assert!(active_report.is_none(), "Tool call should be removed from active tracking");
    }

    #[tokio::test]
    async fn test_tool_call_failure_lifecycle() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_456".into());
        let tool_name = "failing_tool";
        let arguments = json!({"will_fail": true});

        // Create tool call
        let report = handler.create_tool_call_report(&session_id, tool_name, &arguments).await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver.recv().await.expect("Should receive initial notification");

        // Update to in_progress
        handler.update_tool_call_report(&session_id, &tool_call_id, |report| {
            report.update_status(ToolCallStatus::InProgress);
        }).await;

        // Consume progress notification
        let _ = receiver.recv().await.expect("Should receive progress notification");

        // Fail tool call with error
        let error_output = json!({"error": "Tool execution failed", "code": 500});
        let failed_report = handler.fail_tool_call_report(&session_id, &tool_call_id, Some(error_output.clone())).await
            .expect("Should fail successfully");

        // Verify failure notification
        let notification = receiver.recv().await.expect("Should receive failure notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0, tool_call_id);
                assert_eq!(update.fields.status, Some(agent_client_protocol::ToolCallStatus::Failed));
                assert_eq!(update.fields.raw_output, Some(error_output));
            }
            _ => panic!("Expected ToolCallUpdate failure notification"),
        }
    }

    #[tokio::test]
    async fn test_tool_call_cancellation() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_789".into());
        let tool_name = "long_running_tool";
        let arguments = json!({"duration": 3600});

        // Create and start tool call
        let report = handler.create_tool_call_report(&session_id, tool_name, &arguments).await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver.recv().await.expect("Should receive initial notification");

        // Update to in_progress
        handler.update_tool_call_report(&session_id, &tool_call_id, |report| {
            report.update_status(ToolCallStatus::InProgress);
        }).await;

        // Consume progress notification
        let _ = receiver.recv().await.expect("Should receive progress notification");

        // Cancel tool call
        let cancelled_report = handler.cancel_tool_call_report(&session_id, &tool_call_id).await
            .expect("Should cancel successfully");

        // Verify cancellation notification
        let notification = receiver.recv().await.expect("Should receive cancellation notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0, tool_call_id);
                assert_eq!(update.fields.status, Some(agent_client_protocol::ToolCallStatus::Cancelled));
            }
            _ => panic!("Expected ToolCallUpdate cancellation notification"),
        }
    }

    #[tokio::test]
    async fn test_concurrent_tool_execution() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_concurrent".into());

        // Create multiple tool calls concurrently
        let mut tasks = vec![];
        for i in 0..3 {
            let handler_clone = handler.clone();
            let session_clone = session_id.clone();
            let task = tokio::spawn(async move {
                let tool_name = format!("concurrent_tool_{}", i);
                let arguments = json!({"index": i});
                
                let report = handler_clone.create_tool_call_report(&session_clone, &tool_name, &arguments).await;
                let tool_call_id = report.tool_call_id.clone();
                
                // Simulate some work
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                
                // Complete the tool
                handler_clone.complete_tool_call_report(&session_clone, &tool_call_id, Some(json!({"result": i}))).await
            });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(tasks).await;
        for result in results {
            assert!(result.is_ok(), "All concurrent tasks should complete successfully");
        }

        // Verify we received the expected number of notifications
        // Each tool call should generate 2 notifications: initial + completion
        let mut notification_count = 0;
        while let Ok(_) = receiver.try_recv() {
            notification_count += 1;
        }
        assert_eq!(notification_count, 6, "Should receive 6 notifications total (3 initial + 3 completion)");
    }

    #[tokio::test]
    async fn test_tool_call_with_content_and_locations() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_content".into());
        let tool_name = "file_operation_tool";
        let arguments = json!({"file_path": "/test/file.txt"});

        // Create tool call
        let report = handler.create_tool_call_report(&session_id, tool_name, &arguments).await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver.recv().await.expect("Should receive initial notification");

        // Update with content and location
        handler.update_tool_call_report(&session_id, &tool_call_id, |report| {
            report.update_status(ToolCallStatus::InProgress);
            report.add_content(crate::tool_types::ToolCallContent::Content {
                content: agent_client_protocol::ContentBlock::Text(agent_client_protocol::TextContent {
                    text: "Processing file...".to_string(),
                    annotations: None,
                    meta: None,
                }),
            });
            report.add_location(crate::tool_types::ToolCallLocation {
                path: "/test/file.txt".to_string(),
                line: Some(42),
            });
        }).await;

        // Verify notification with content and location
        let notification = receiver.recv().await.expect("Should receive update notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0, tool_call_id);
                assert!(update.fields.content.is_some(), "Should include content");
                assert!(update.fields.locations.is_some(), "Should include locations");
                
                let locations = update.fields.locations.unwrap();
                assert_eq!(locations.len(), 1);
                assert_eq!(locations[0].path, "/test/file.txt");
                assert_eq!(locations[0].line, Some(42));
            }
            _ => panic!("Expected ToolCallUpdate notification with content"),
        }
    }

    #[tokio::test]
    async fn test_notification_sender_failure_resilience() {
        // Create handler but don't set notification sender
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["test_tool".to_string()],
            forbidden_paths: vec![],
        };
        
        let handler = ToolCallHandler::new(permissions);
        let session_id = SessionId("test_session_no_sender".into());

        // Tool call operations should still work without notification sender
        let report = handler.create_tool_call_report(&session_id, "test_tool", &json!({})).await;
        assert_eq!(report.status, ToolCallStatus::Pending);

        let updated = handler.update_tool_call_report(&session_id, &report.tool_call_id, |r| {
            r.update_status(ToolCallStatus::InProgress);
        }).await;
        assert!(updated.is_some());

        let completed = handler.complete_tool_call_report(&session_id, &report.tool_call_id, None).await;
        assert!(completed.is_some());
    }
}