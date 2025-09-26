//! Session management system for tracking conversation contexts and state

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use ulid::Ulid;

/// Unique identifier for sessions
pub type SessionId = Ulid;

/// A conversation session containing context and metadata
#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub context: Vec<Message>,
    pub client_capabilities: Option<agent_client_protocol::ClientCapabilities>,
    pub mcp_servers: Vec<String>,
}

impl Session {
    /// Create a new session with the given ID
    pub fn new(id: SessionId) -> Self {
        let now = SystemTime::now();
        Self {
            id,
            created_at: now,
            last_accessed: now,
            context: Vec::new(),
            client_capabilities: None,
            mcp_servers: Vec::new(),
        }
    }

    /// Add a message to the session context
    pub fn add_message(&mut self, message: Message) {
        self.context.push(message);
        self.last_accessed = SystemTime::now();
    }

    /// Update the last accessed time
    pub fn update_access_time(&mut self) {
        self.last_accessed = SystemTime::now();
    }
}

/// A message within a session context
#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: SystemTime,
}

impl Message {
    /// Create a new message
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            role,
            content,
            timestamp: SystemTime::now(),
        }
    }
}

/// Role of a message sender
#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Thread-safe session manager
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    cleanup_interval: Duration,
    max_session_age: Duration,
}

impl SessionManager {
    /// Create a new session manager with default settings
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            max_session_age: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Create a new session manager with custom cleanup settings
    pub fn with_cleanup_settings(cleanup_interval: Duration, max_session_age: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval,
            max_session_age,
        }
    }

    /// Create a new session and return its ID
    pub fn create_session(&self) -> crate::Result<SessionId> {
        let session_id = Ulid::new();
        let session = Session::new(session_id);

        let mut sessions = self.sessions.write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        sessions.insert(session_id, session);
        tracing::debug!("Created new session: {}", session_id);
        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        let sessions = self.sessions.read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        Ok(sessions.get(session_id).cloned())
    }

    /// Update a session using a closure
    pub fn update_session<F>(&self, session_id: &SessionId, updater: F) -> crate::Result<()>
    where
        F: FnOnce(&mut Session),
    {
        let mut sessions = self.sessions.write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        if let Some(session) = sessions.get_mut(session_id) {
            updater(session);
            session.update_access_time();
            tracing::debug!("Updated session: {}", session_id);
        } else {
            tracing::warn!("Attempted to update non-existent session: {}", session_id);
        }

        Ok(())
    }

    /// Remove a session and return it if it existed
    pub fn remove_session(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        let mut sessions = self.sessions.write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        let removed = sessions.remove(session_id);
        if removed.is_some() {
            tracing::debug!("Removed session: {}", session_id);
        }
        Ok(removed)
    }

    /// List all session IDs
    pub fn list_sessions(&self) -> crate::Result<Vec<SessionId>> {
        let sessions = self.sessions.read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        Ok(sessions.keys().cloned().collect())
    }

    /// Get the number of active sessions
    pub fn session_count(&self) -> crate::Result<usize> {
        let sessions = self.sessions.read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        Ok(sessions.len())
    }

    /// Start the cleanup task that removes expired sessions
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let manager = Arc::clone(&self);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(manager.cleanup_interval);

            tracing::info!("Session cleanup task started with interval: {:?}", manager.cleanup_interval);

            loop {
                interval.tick().await;
                if let Err(e) = manager.cleanup_expired_sessions().await {
                    tracing::error!("Session cleanup failed: {}", e);
                }
            }
        });
    }

    /// Clean up expired sessions
    async fn cleanup_expired_sessions(&self) -> crate::Result<()> {
        let now = SystemTime::now();
        let mut expired_sessions = Vec::new();

        // Find expired sessions
        {
            let sessions = self.sessions.read()
                .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

            for (id, session) in sessions.iter() {
                if let Ok(age) = now.duration_since(session.last_accessed) {
                    if age > self.max_session_age {
                        expired_sessions.push(*id);
                    }
                }
            }
        }

        // Remove expired sessions
        for session_id in expired_sessions {
            tracing::info!("Cleaning up expired session: {}", session_id);
            self.remove_session(&session_id)?;
        }

        Ok(())
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_session_creation() {
        let session_id = Ulid::new();
        let session = Session::new(session_id);

        assert_eq!(session.id, session_id);
        assert!(session.context.is_empty());
        assert!(session.client_capabilities.is_none());
        assert!(session.mcp_servers.is_empty());
    }

    #[test]
    fn test_message_creation() {
        let message = Message::new(MessageRole::User, "Hello".to_string());
        
        assert!(matches!(message.role, MessageRole::User));
        assert_eq!(message.content, "Hello");
    }

    #[test]
    fn test_session_add_message() {
        let session_id = Ulid::new();
        let mut session = Session::new(session_id);
        let initial_time = session.last_accessed;

        // Small delay to ensure time difference
        std::thread::sleep(Duration::from_millis(1));

        let message = Message::new(MessageRole::User, "Hello".to_string());
        session.add_message(message);

        assert_eq!(session.context.len(), 1);
        assert!(session.last_accessed > initial_time);
    }

    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::new();
        assert_eq!(manager.cleanup_interval, Duration::from_secs(300));
        assert_eq!(manager.max_session_age, Duration::from_secs(3600));
    }

    #[test]
    fn test_session_manager_with_custom_settings() {
        let cleanup_interval = Duration::from_secs(60);
        let max_age = Duration::from_secs(1800);
        let manager = SessionManager::with_cleanup_settings(cleanup_interval, max_age);
        
        assert_eq!(manager.cleanup_interval, cleanup_interval);
        assert_eq!(manager.max_session_age, max_age);
    }

    #[test]
    fn test_create_and_get_session() {
        let manager = SessionManager::new();

        let session_id = manager.create_session().unwrap();
        let session = manager.get_session(&session_id).unwrap();

        assert!(session.is_some());
        assert_eq!(session.unwrap().id, session_id);
    }

    #[test]
    fn test_get_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = Ulid::new();

        let session = manager.get_session(&nonexistent_id).unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn test_update_session() {
        let manager = SessionManager::new();
        let session_id = manager.create_session().unwrap();

        let message = Message::new(MessageRole::User, "Hello".to_string());

        manager.update_session(&session_id, |session| {
            session.add_message(message.clone());
        }).unwrap();

        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.context.len(), 1);
        assert_eq!(session.context[0].content, "Hello");
    }

    #[test]
    fn test_update_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = Ulid::new();

        // Should not panic when trying to update a non-existent session
        let result = manager.update_session(&nonexistent_id, |session| {
            session.add_message(Message::new(MessageRole::User, "test".to_string()));
        });

        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_session() {
        let manager = SessionManager::new();
        let session_id = manager.create_session().unwrap();

        // Verify session exists
        assert!(manager.get_session(&session_id).unwrap().is_some());

        // Remove session
        let removed = manager.remove_session(&session_id).unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, session_id);

        // Verify session no longer exists
        assert!(manager.get_session(&session_id).unwrap().is_none());
    }

    #[test]
    fn test_remove_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = Ulid::new();

        let removed = manager.remove_session(&nonexistent_id).unwrap();
        assert!(removed.is_none());
    }

    #[test]
    fn test_list_sessions() {
        let manager = SessionManager::new();

        // Initially empty
        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 0);

        // Create some sessions
        let id1 = manager.create_session().unwrap();
        let id2 = manager.create_session().unwrap();

        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&id1));
        assert!(sessions.contains(&id2));
    }

    #[test]
    fn test_session_count() {
        let manager = SessionManager::new();

        assert_eq!(manager.session_count().unwrap(), 0);

        manager.create_session().unwrap();
        assert_eq!(manager.session_count().unwrap(), 1);

        manager.create_session().unwrap();
        assert_eq!(manager.session_count().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_cleanup_expired_sessions() {
        // Create manager with very short expiration time
        let manager = Arc::new(SessionManager::with_cleanup_settings(
            Duration::from_millis(100),
            Duration::from_millis(50), // 50ms max age
        ));

        // Create a session
        let session_id = manager.create_session().unwrap();
        assert_eq!(manager.session_count().unwrap(), 1);

        // Wait for session to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Manually trigger cleanup
        manager.cleanup_expired_sessions().await.unwrap();

        // Session should be removed
        assert_eq!(manager.session_count().unwrap(), 0);
        assert!(manager.get_session(&session_id).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cleanup_task_startup() {
        let manager = Arc::new(SessionManager::new());
        
        // This should not panic or block
        manager.clone().start_cleanup_task().await;
        
        // Give the task a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}