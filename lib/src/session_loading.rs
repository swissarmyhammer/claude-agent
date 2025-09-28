//! Enhanced session loading with comprehensive ACP-compliant error handling
//!
//! This module provides session loading functionality with detailed error handling
//! for all failure scenarios specified in the ACP specification.

use crate::{
    session::{Session, SessionManager},
    session_errors::{SessionSetupError, SessionSetupResult},
    session_validation::validate_session_id,
};
use agent_client_protocol::{
    ContentBlock, LoadSessionRequest, LoadSessionResponse, SessionNotification, SessionUpdate,
    TextContent,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

/// Enhanced session loader with comprehensive error handling
pub struct EnhancedSessionLoader {
    session_manager: SessionManager,
    max_session_age: Duration,
    enable_history_replay: bool,
    max_history_messages: usize,
}

impl EnhancedSessionLoader {
    /// Create a new enhanced session loader
    pub fn new(session_manager: SessionManager) -> Self {
        Self {
            session_manager,
            max_session_age: Duration::from_secs(24 * 60 * 60), // 24 hours
            enable_history_replay: true,
            max_history_messages: 10000,
        }
    }

    /// Create a new enhanced session loader with custom settings
    pub fn with_settings(
        session_manager: SessionManager,
        max_session_age: Duration,
        enable_history_replay: bool,
        max_history_messages: usize,
    ) -> Self {
        Self {
            session_manager,
            max_session_age,
            enable_history_replay,
            max_history_messages,
        }
    }

    /// Load a session with comprehensive error handling and validation
    ///
    /// This method implements all ACP requirements for session loading:
    /// 1. Validates session ID format
    /// 2. Checks session existence and expiration
    /// 3. Validates session data integrity
    /// 4. Handles storage backend failures
    /// 5. Provides detailed error information for all failure scenarios
    pub async fn load_session_enhanced(
        &self,
        request: &LoadSessionRequest,
        capabilities_load_session: bool,
    ) -> SessionSetupResult<(Session, Vec<SessionNotification>)> {
        // Step 1: Validate loadSession capability
        if !capabilities_load_session {
            warn!("Session load requested but loadSession capability not supported");
            return Err(SessionSetupError::LoadSessionNotSupported {
                declared_capability: false,
            });
        }

        // Step 2: Validate session ID format
        let session_id = validate_session_id(&request.session_id.0)?;

        info!("Loading session with enhanced validation: {}", session_id);

        // Step 3: Attempt to retrieve session with storage error handling
        let session = match self.session_manager.get_session(&session_id) {
            Ok(session_option) => session_option,
            Err(e) => {
                error!("Session storage failure while loading {}: {}", session_id, e);
                return Err(SessionSetupError::SessionStorageFailure {
                    session_id: Some(agent_client_protocol::SessionId(session_id.to_string().into())),
                    storage_error: e.to_string(),
                    recovery_suggestion: "Check session storage backend and retry".to_string(),
                });
            }
        };

        // Step 4: Handle session not found
        let session = match session {
            Some(session) => session,
            None => {
                warn!("Session not found: {}", session_id);
                
                // Get list of available sessions for better error reporting
                let available_sessions = self.get_available_session_list()?;
                
                return Err(SessionSetupError::SessionNotFound {
                    session_id: agent_client_protocol::SessionId(session_id.to_string().into()),
                    available_sessions,
                });
            }
        };

        // Step 5: Check session expiration
        let now = SystemTime::now();
        if let Ok(age) = now.duration_since(session.last_accessed) {
            if age > self.max_session_age {
                warn!("Session expired: {} (age: {:?})", session_id, age);
                
                let expired_at = session.last_accessed
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                return Err(SessionSetupError::SessionExpired {
                    session_id: agent_client_protocol::SessionId(session_id.to_string().into()),
                    expired_at: chrono::DateTime::from_timestamp(expired_at as i64, 0)
                        .unwrap_or_default()
                        .to_rfc3339(),
                    max_age_seconds: self.max_session_age.as_secs(),
                });
            }
        }

        // Step 6: Validate session data integrity
        self.validate_session_integrity(&session)?;

        // Step 7: Prepare history replay notifications if enabled
        let history_notifications = if self.enable_history_replay {
            self.prepare_history_replay(&session).await?
        } else {
            Vec::new()
        };

        info!(
            "Successfully loaded session {} with {} historical messages",
            session_id,
            session.context.len()
        );

        Ok((session, history_notifications))
    }

    /// Validate session data integrity
    fn validate_session_integrity(&self, session: &Session) -> SessionSetupResult<()> {
        // Check for basic data corruption indicators
        
        // Validate timestamps
        if session.created_at > SystemTime::now() {
            return Err(SessionSetupError::SessionCorrupted {
                session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                corruption_details: "Session created_at timestamp is in the future".to_string(),
            });
        }

        if session.last_accessed > SystemTime::now() {
            return Err(SessionSetupError::SessionCorrupted {
                session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                corruption_details: "Session last_accessed timestamp is in the future".to_string(),
            });
        }

        if session.created_at > session.last_accessed {
            return Err(SessionSetupError::SessionCorrupted {
                session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                corruption_details: "Session created_at is after last_accessed".to_string(),
            });
        }

        // Validate message integrity
        for (i, message) in session.context.iter().enumerate() {
            if message.content.is_empty() {
                warn!("Empty message content found at index {} in session {}", i, session.id);
                // Don't fail for empty messages, just log warning
            }

            if message.timestamp > SystemTime::now() {
                return Err(SessionSetupError::SessionCorrupted {
                    session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                    corruption_details: format!("Message {} timestamp is in the future", i),
                });
            }
        }

        // Check for excessive message count
        if session.context.len() > self.max_history_messages {
            return Err(SessionSetupError::SessionCorrupted {
                session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                corruption_details: format!(
                    "Session contains {} messages, exceeding maximum of {}",
                    session.context.len(),
                    self.max_history_messages
                ),
            });
        }

        Ok(())
    }

    /// Prepare history replay notifications with error handling
    async fn prepare_history_replay(&self, session: &Session) -> SessionSetupResult<Vec<SessionNotification>> {
        if session.context.is_empty() {
            return Ok(Vec::new());
        }

        info!(
            "Preparing history replay for {} messages in session {}",
            session.context.len(),
            session.id
        );

        let mut notifications = Vec::new();

        for (i, message) in session.context.iter().enumerate() {
            // Create session update based on message role
            let session_update = match message.role {
                crate::session::MessageRole::User => {
                    SessionUpdate::UserMessageChunk {
                        content: ContentBlock::Text(TextContent {
                            text: message.content.clone(),
                            annotations: None,
                            meta: None,
                        }),
                    }
                }
                crate::session::MessageRole::Assistant | crate::session::MessageRole::System => {
                    SessionUpdate::AgentMessageChunk {
                        content: ContentBlock::Text(TextContent {
                            text: message.content.clone(),
                            annotations: None,
                            meta: None,
                        }),
                    }
                }
            };

            // Create notification with metadata
            let notification = SessionNotification {
                session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                update: session_update,
                meta: Some(serde_json::json!({
                    "timestamp": message.timestamp
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    "message_type": "historical_replay",
                    "message_index": i,
                    "total_messages": session.context.len(),
                    "original_role": format!("{:?}", message.role),
                    "session_age": SystemTime::now()
                        .duration_since(session.created_at)
                        .unwrap_or_default()
                        .as_secs()
                })),
            };

            notifications.push(notification);

            // Check for potential issues during replay preparation
            if i > 0 && i % 1000 == 0 {
                info!("Prepared {} of {} history messages for replay", i, session.context.len());
            }
        }

        info!(
            "Successfully prepared {} history notifications for session {}",
            notifications.len(),
            session.id
        );

        Ok(notifications)
    }

    /// Get list of available sessions for error reporting
    fn get_available_session_list(&self) -> SessionSetupResult<Vec<String>> {
        match self.session_manager.list_sessions() {
            Ok(session_ids) => Ok(session_ids
                .into_iter()
                .take(10) // Limit to first 10 sessions for error message
                .map(|id| id.to_string())
                .collect()),
            Err(e) => {
                error!("Failed to list sessions for error reporting: {}", e);
                // Don't fail the whole operation just because we can't list sessions
                Ok(vec!["Unable to list available sessions".to_string()])
            }
        }
    }

    /// Validate session loading request parameters
    pub fn validate_load_request(&self, request: &LoadSessionRequest) -> SessionSetupResult<()> {
        // Validate session ID format
        validate_session_id(&request.session_id.0)?;

        // Validate working directory (always present in ACP)
        crate::session_validation::validate_working_directory(&request.cwd)?;

        // For now, we'll skip MCP server validation as the types don't match
        // TODO: Add proper MCP server validation once types are aligned

        Ok(())
    }

    /// Create enhanced LoadSessionResponse with proper metadata
    pub fn create_load_response(&self, session: &Session, request: &LoadSessionRequest) -> LoadSessionResponse {
        LoadSessionResponse {
            modes: None, // No specific modes for now
            meta: Some(serde_json::json!({
                "session_id": session.id.to_string(),
                "created_at": session.created_at
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "last_accessed": session.last_accessed
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "message_count": session.context.len(),
                "client_capabilities": session.client_capabilities.is_some(),
                "mcp_servers": session.mcp_servers.clone(),
                "requested_cwd": request.cwd.display().to_string(),
                "requested_mcp_servers": request.mcp_servers.len(),
                "load_timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })),
        }
    }
}

/// Session history replay manager with error recovery
pub struct SessionHistoryReplayer {
    max_replay_failures: usize,
    replay_delay_ms: u64,
}

impl Default for SessionHistoryReplayer {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionHistoryReplayer {
    /// Create a new session history replayer
    pub fn new() -> Self {
        Self {
            max_replay_failures: 5,
            replay_delay_ms: 10,
        }
    }

    /// Replay session history with error handling and recovery
    pub async fn replay_history_with_recovery(
        &self,
        session: &Session,
        notification_sender: &dyn SessionNotificationSender,
    ) -> SessionSetupResult<()> {
        if session.context.is_empty() {
            return Ok(());
        }

        let mut failure_count = 0;
        let total_messages = session.context.len();

        info!(
            "Starting history replay for session {} ({} messages)",
            session.id, total_messages
        );

        for (i, message) in session.context.iter().enumerate() {
            let session_update = match message.role {
                crate::session::MessageRole::User => {
                    SessionUpdate::UserMessageChunk {
                        content: ContentBlock::Text(TextContent {
                            text: message.content.clone(),
                            annotations: None,
                            meta: None,
                        }),
                    }
                }
                crate::session::MessageRole::Assistant | crate::session::MessageRole::System => {
                    SessionUpdate::AgentMessageChunk {
                        content: ContentBlock::Text(TextContent {
                            text: message.content.clone(),
                            annotations: None,
                            meta: None,
                        }),
                    }
                }
            };

            let notification = SessionNotification {
                session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                update: session_update,
                meta: Some(serde_json::json!({
                    "timestamp": message.timestamp
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    "message_type": "historical_replay",
                    "message_index": i,
                    "total_messages": total_messages,
                    "original_role": format!("{:?}", message.role)
                })),
            };

            // Send notification with error handling
            match notification_sender.send_notification(notification).await {
                Ok(()) => {
                    // Reset failure count on success
                    failure_count = 0;

                    // Add small delay to avoid overwhelming the client
                    if self.replay_delay_ms > 0 {
                        tokio::time::sleep(Duration::from_millis(self.replay_delay_ms)).await;
                    }
                }
                Err(e) => {
                    failure_count += 1;
                    error!("Failed to send history message {} of {}: {}", i + 1, total_messages, e);

                    if failure_count >= self.max_replay_failures {
                        error!("Too many replay failures ({}), aborting history replay", failure_count);
                        return Err(SessionSetupError::SessionHistoryReplayFailed {
                            session_id: agent_client_protocol::SessionId(session.id.to_string().into()),
                            failed_at_message: i,
                            total_messages,
                            error_details: format!(
                                "Exceeded maximum replay failures ({}): {}",
                                self.max_replay_failures, e
                            ),
                        });
                    }

                    // Exponential backoff on failures
                    let delay = Duration::from_millis(self.replay_delay_ms * (1 << failure_count));
                    tokio::time::sleep(delay).await;
                }
            }

            // Progress reporting for large sessions
            if i > 0 && (i + 1) % 100 == 0 {
                info!("Replayed {} of {} messages", i + 1, total_messages);
            }
        }

        info!(
            "Successfully completed history replay for session {} ({} messages)",
            session.id, total_messages
        );

        Ok(())
    }
}

/// Trait for sending session notifications with error handling
#[async_trait::async_trait]
pub trait SessionNotificationSender {
    /// Send a session notification
    async fn send_notification(&self, notification: SessionNotification) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Message, MessageRole};
    use std::time::Duration;

    fn create_test_session() -> Session {
        let session_id = ulid::Ulid::new();
        let mut session = Session::new(session_id);
        
        // Add some test messages
        session.add_message(Message::new(
            MessageRole::User,
            "Hello, world!".to_string(),
        ));
        
        session.add_message(Message::new(
            MessageRole::Assistant,
            "Hello! How can I help you?".to_string(),
        ));
        
        session
    }

    #[tokio::test]
    async fn test_enhanced_session_loader_creation() {
        let session_manager = SessionManager::new();
        let loader = EnhancedSessionLoader::new(session_manager);
        assert!(loader.enable_history_replay);
        assert_eq!(loader.max_history_messages, 10000);
    }

    #[test]
    fn test_validate_session_integrity_valid() {
        let session = create_test_session();
        let session_manager = SessionManager::new();
        let loader = EnhancedSessionLoader::new(session_manager);
        
        let result = loader.validate_session_integrity(&session);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_session_integrity_future_timestamp() {
        let session_id = ulid::Ulid::new();
        let mut session = Session::new(session_id);
        
        // Add message with future timestamp
        let mut message = Message::new(MessageRole::User, "test".to_string());
        message.timestamp = SystemTime::now() + Duration::from_secs(3600);
        session.context.push(message);
        
        let session_manager = SessionManager::new();
        let loader = EnhancedSessionLoader::new(session_manager);
        
        let result = loader.validate_session_integrity(&session);
        assert!(result.is_err());
        
        if let Err(SessionSetupError::SessionCorrupted { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected SessionCorrupted error");
        }
    }

    #[tokio::test]
    async fn test_prepare_history_replay() {
        let session = create_test_session();
        let session_manager = SessionManager::new();
        let loader = EnhancedSessionLoader::new(session_manager);
        
        let notifications = loader.prepare_history_replay(&session).await.unwrap();
        assert_eq!(notifications.len(), 2);
        
        // Check that notifications contain proper metadata
        for notification in &notifications {
            assert!(notification.meta.is_some());
            let meta = notification.meta.as_ref().unwrap();
            assert!(meta.get("message_type").is_some());
            assert!(meta.get("message_index").is_some());
        }
    }

    #[test]
    fn test_validate_session_id_valid() {
        let valid_ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let result = validate_session_id(valid_ulid);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_session_id_invalid() {
        let invalid_id = "not-a-valid-session-id";
        let result = validate_session_id(invalid_id);
        
        assert!(result.is_err());
        if let Err(SessionSetupError::InvalidSessionId { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidSessionId error");
        }
    }

    #[tokio::test]
    async fn test_session_history_replayer() {
        let replayer = SessionHistoryReplayer::new();
        assert_eq!(replayer.max_replay_failures, 5);
        assert_eq!(replayer.replay_delay_ms, 10);
    }
}