//! Session management system for tracking conversation contexts and state

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use ulid::Ulid;

/// Session identifier with ACP-compliant format
///
/// # Format
/// `sess_<ULID>`
///
/// # Example
/// `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV`
///
/// # ACP Requirements
/// 1. Unique identifier for conversation context
/// 2. Must persist across session loads
/// 3. Used in session/prompt, session/cancel, session/load
/// 4. Should follow consistent, recognizable format
/// 5. Must be URL-safe and filesystem-safe
///
/// The `sess_` prefix provides human recognition while ULID provides:
/// - Cryptographic uniqueness
/// - Sortability by creation time
/// - URL-safe and filesystem-safe characters
/// - 128-bit entropy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Ulid);

impl SessionId {
    /// Prefix for session IDs
    pub const PREFIX: &'static str = "sess_";

    /// Create a new session ID with proper format
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    /// Parse a session ID from string
    ///
    /// # Format
    /// Expects format: `sess_<ULID>`
    ///
    /// # Errors
    /// Returns error if:
    /// - Missing `sess_` prefix
    /// - Invalid ULID after prefix
    /// - Empty string
    pub fn parse(s: &str) -> Result<Self, SessionIdError> {
        if s.is_empty() {
            return Err(SessionIdError::Empty);
        }

        if !s.starts_with(Self::PREFIX) {
            return Err(SessionIdError::MissingPrefix {
                provided: s.to_string(),
                expected_prefix: Self::PREFIX.to_string(),
            });
        }

        let ulid_str = &s[Self::PREFIX.len()..];
        if ulid_str.is_empty() {
            return Err(SessionIdError::MissingUlid {
                provided: s.to_string(),
            });
        }

        match Ulid::from_string(ulid_str) {
            Ok(ulid) => Ok(Self(ulid)),
            Err(e) => Err(SessionIdError::InvalidUlid {
                provided: ulid_str.to_string(),
                error: e.to_string(),
            }),
        }
    }

    /// Get the underlying ULID
    pub fn as_ulid(&self) -> Ulid {
        self.0
    }

    /// Get the raw ULID string without the sess_ prefix
    ///
    /// This is useful for backward compatibility with internal storage systems
    /// or when interacting with legacy code that expects raw ULIDs. The ACP
    /// protocol format (sess_<ULID>) is maintained for external communication,
    /// while this method allows access to just the ULID portion for internal use.
    ///
    /// # Example
    /// ```ignore
    /// let session_id = SessionId::parse("sess_01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    /// assert_eq!(session_id.ulid_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    /// ```
    pub fn ulid_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}

impl FromStr for SessionId {
    type Err = SessionIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<Ulid> for SessionId {
    fn from(ulid: Ulid) -> Self {
        Self(ulid)
    }
}

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Errors that can occur when parsing session IDs
#[derive(Debug, Clone, thiserror::Error)]
pub enum SessionIdError {
    /// Session ID is an empty string
    ///
    /// This error occurs when attempting to parse an empty string as a session ID.
    /// Provide a properly formatted session ID with the pattern: sess_<ULID>
    #[error("Session ID cannot be empty")]
    Empty,

    /// Session ID is missing the required 'sess_' prefix
    ///
    /// This error occurs when the session ID does not start with 'sess_'.
    /// The ACP protocol requires all session IDs to have a recognizable prefix
    /// for consistent identification and validation.
    ///
    /// # Example
    /// - Invalid: `01ARZ3NDEKTSV4RRFFQ69G5FAV`
    /// - Valid: `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV`
    #[error("Session ID missing required prefix '{expected_prefix}': got '{provided}'")]
    MissingPrefix {
        provided: String,
        expected_prefix: String,
    },

    /// Session ID has the prefix but no ULID follows
    ///
    /// This error occurs when the session ID is exactly 'sess_' with no ULID portion.
    /// The session ID must include a valid 26-character ULID after the prefix.
    ///
    /// # Example
    /// - Invalid: `sess_`
    /// - Valid: `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV`
    #[error("Session ID missing ULID after prefix: got '{provided}'")]
    MissingUlid { provided: String },

    /// The ULID portion of the session ID is malformed
    ///
    /// This error occurs when the characters after 'sess_' do not form a valid ULID.
    /// ULIDs must be exactly 26 characters using Crockford's Base32 encoding
    /// (0-9, A-Z excluding I, L, O, U).
    ///
    /// # Example
    /// - Invalid: `sess_INVALID` (too short)
    /// - Invalid: `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV!!!` (invalid characters)
    /// - Valid: `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV`
    #[error("Invalid ULID format in session ID '{provided}': {error}")]
    InvalidUlid { provided: String, error: String },
}

/// A conversation session containing context and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub context: Vec<Message>,
    pub client_capabilities: Option<agent_client_protocol::ClientCapabilities>,
    pub mcp_servers: Vec<String>,
    /// Working directory for this session (ACP requirement - must be absolute path)
    pub cwd: PathBuf,
    /// Available commands that can be invoked during this session
    pub available_commands: Vec<agent_client_protocol::AvailableCommand>,
    /// Number of language model requests made in the current turn
    pub turn_request_count: u64,
    /// Total tokens consumed in the current turn (input + output)
    pub turn_token_count: u64,
    /// Current session mode identifier for ACP current mode updates
    pub current_mode: Option<String>,
}

impl Session {
    /// Create a new session with the given ID and working directory
    ///
    /// # Arguments
    /// * `id` - Unique session identifier (ULID)
    /// * `cwd` - Working directory for the session (must be absolute path as per ACP spec)
    ///
    /// # Panics
    /// This function will panic if the working directory is not absolute, as this violates
    /// the ACP specification requirement that sessions must have absolute working directories.
    pub fn new(id: SessionId, cwd: PathBuf) -> Self {
        // ACP requires absolute working directory - validate this at session creation
        if !cwd.is_absolute() {
            panic!(
                "Session working directory must be absolute path (ACP requirement), got: {}",
                cwd.display()
            );
        }

        let now = SystemTime::now();
        Self {
            id,
            created_at: now,
            last_accessed: now,
            context: Vec::new(),
            client_capabilities: None,
            mcp_servers: Vec::new(),
            cwd,
            available_commands: Vec::new(),
            turn_request_count: 0,
            turn_token_count: 0,
            current_mode: None,
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

    /// Update available commands for this session
    pub fn update_available_commands(
        &mut self,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) {
        self.available_commands = commands;
        self.last_accessed = SystemTime::now();
    }

    /// Check if available commands have changed from the given set
    pub fn has_available_commands_changed(
        &self,
        new_commands: &[agent_client_protocol::AvailableCommand],
    ) -> bool {
        if self.available_commands.len() != new_commands.len() {
            return true;
        }

        // Compare each command by name and description
        for (existing, new) in self.available_commands.iter().zip(new_commands.iter()) {
            if existing.name != new.name || existing.description != new.description {
                return true;
            }
        }

        false
    }

    /// Reset turn counters for a new turn
    pub fn reset_turn_counters(&mut self) {
        self.turn_request_count = 0;
        self.turn_token_count = 0;
        self.last_accessed = SystemTime::now();
    }

    /// Increment the turn request count and return the new value
    pub fn increment_turn_requests(&mut self) -> u64 {
        self.turn_request_count += 1;
        self.last_accessed = SystemTime::now();
        self.turn_request_count
    }

    /// Add tokens to the current turn count and return the new total
    pub fn add_turn_tokens(&mut self, tokens: u64) -> u64 {
        self.turn_token_count += tokens;
        self.last_accessed = SystemTime::now();
        self.turn_token_count
    }

    /// Get the current turn request count
    pub fn get_turn_request_count(&self) -> u64 {
        self.turn_request_count
    }

    /// Get the current turn token count
    pub fn get_turn_token_count(&self) -> u64 {
        self.turn_token_count
    }
}

/// A message within a session context
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Thread-safe session manager
#[derive(Debug)]
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

    /// Create a new session with specified working directory and return its ID
    ///
    /// # Arguments
    /// * `cwd` - Working directory for the session (must be absolute path as per ACP spec)
    ///
    /// # Errors
    /// Returns error if:
    /// - Working directory validation fails
    /// - Session storage write lock cannot be acquired
    pub fn create_session(&self, cwd: PathBuf) -> crate::Result<SessionId> {
        // Validate working directory before creating session
        crate::session_validation::validate_working_directory(&cwd).map_err(|e| {
            crate::AgentError::Session(format!("Working directory validation failed: {}", e))
        })?;

        let session_id = SessionId::new();
        let session = Session::new(session_id, cwd);

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        sessions.insert(session_id, session);
        tracing::debug!("Created new session: {}", session_id);
        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        Ok(sessions.get(session_id).cloned())
    }

    /// Update a session using a closure
    pub fn update_session<F>(&self, session_id: &SessionId, updater: F) -> crate::Result<()>
    where
        F: FnOnce(&mut Session),
    {
        let mut sessions = self
            .sessions
            .write()
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
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        let removed = sessions.remove(session_id);
        if removed.is_some() {
            tracing::debug!("Removed session: {}", session_id);
        }
        Ok(removed)
    }

    /// List all session IDs
    pub fn list_sessions(&self) -> crate::Result<Vec<SessionId>> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        Ok(sessions.keys().cloned().collect())
    }

    /// Get the number of active sessions
    pub fn session_count(&self) -> crate::Result<usize> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        Ok(sessions.len())
    }

    /// Update available commands for a session and return whether an update was sent
    /// Returns true if commands changed and update was needed, false if no change
    pub fn update_available_commands(
        &self,
        session_id: &SessionId,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) -> crate::Result<bool> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        if let Some(session) = sessions.get_mut(session_id) {
            let commands_changed = session.has_available_commands_changed(&commands);
            if commands_changed {
                session.update_available_commands(commands);
                tracing::debug!("Updated available commands for session: {}", session_id);
                Ok(true)
            } else {
                tracing::trace!("Available commands unchanged for session: {}", session_id);
                Ok(false)
            }
        } else {
            tracing::warn!(
                "Attempted to update commands for non-existent session: {}",
                session_id
            );
            Ok(false)
        }
    }

    /// Start the cleanup task that removes expired sessions
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let manager = Arc::clone(&self);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(manager.cleanup_interval);

            tracing::info!(
                "Session cleanup task started with interval: {:?}",
                manager.cleanup_interval
            );

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
            let sessions = self.sessions.read().map_err(|_| {
                crate::AgentError::Session("Failed to acquire read lock".to_string())
            })?;

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

    // SessionId tests
    #[test]
    fn test_session_id_new() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();

        // Should be different
        assert_ne!(id1, id2);

        // Should have correct format
        let id_str = id1.to_string();
        assert!(id_str.starts_with("sess_"));
        assert_eq!(id_str.len(), 5 + 26); // "sess_" + 26-char ULID
    }

    #[test]
    fn test_session_id_parse_valid() {
        let valid_id = "sess_01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let result = SessionId::parse(valid_id);
        assert!(result.is_ok());

        let session_id = result.unwrap();
        assert_eq!(session_id.to_string(), valid_id);
    }

    #[test]
    fn test_session_id_parse_missing_prefix() {
        let invalid_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let result = SessionId::parse(invalid_id);
        assert!(result.is_err());

        match result {
            Err(SessionIdError::MissingPrefix { .. }) => {}
            _ => panic!("Expected MissingPrefix error"),
        }
    }

    #[test]
    fn test_session_id_parse_invalid_ulid() {
        let invalid_id = "sess_invalid-ulid-format";
        let result = SessionId::parse(invalid_id);
        assert!(result.is_err());

        match result {
            Err(SessionIdError::InvalidUlid { .. }) => {}
            _ => panic!("Expected InvalidUlid error"),
        }
    }

    #[test]
    fn test_session_id_parse_empty() {
        let result = SessionId::parse("");
        assert!(result.is_err());

        match result {
            Err(SessionIdError::Empty) => {}
            _ => panic!("Expected Empty error"),
        }
    }

    #[test]
    fn test_session_id_parse_missing_ulid() {
        let invalid_id = "sess_";
        let result = SessionId::parse(invalid_id);
        assert!(result.is_err());

        match result {
            Err(SessionIdError::MissingUlid { .. }) => {}
            _ => panic!("Expected MissingUlid error"),
        }
    }

    #[test]
    fn test_session_id_serialization() {
        let session_id = SessionId::new();
        let serialized = serde_json::to_string(&session_id).unwrap();

        // Should serialize as a string with sess_ prefix
        assert!(serialized.contains("sess_"));

        // Should be able to deserialize back
        let deserialized: SessionId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(session_id, deserialized);
    }

    #[test]
    fn test_session_id_from_ulid() {
        let ulid = Ulid::new();
        let session_id = SessionId::from(ulid);

        assert_eq!(session_id.as_ulid(), ulid);
        assert!(session_id.to_string().starts_with("sess_"));
    }

    #[test]
    fn test_session_id_display() {
        let ulid = Ulid::from_string("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        let session_id = SessionId::from(ulid);

        assert_eq!(session_id.to_string(), "sess_01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn test_session_id_from_str() {
        use std::str::FromStr;

        let valid_id = "sess_01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let session_id = SessionId::from_str(valid_id).unwrap();

        assert_eq!(session_id.to_string(), valid_id);
    }

    #[test]
    fn test_session_id_url_safe() {
        let session_id = SessionId::new();
        let id_str = session_id.to_string();

        // Check that it only contains URL-safe characters
        assert!(id_str
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    // Session tests
    #[test]
    fn test_session_creation() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let session = Session::new(session_id, cwd.clone());

        assert_eq!(session.id, session_id);
        assert_eq!(session.cwd, cwd);
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
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);
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
        let cwd = std::env::current_dir().unwrap();

        let session_id = manager.create_session(cwd.clone()).unwrap();
        let session = manager.get_session(&session_id).unwrap();

        assert!(session.is_some());
        let session = session.unwrap();
        assert_eq!(session.id, session_id);
        assert_eq!(session.cwd, cwd);
    }

    #[test]
    fn test_get_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = SessionId::new();

        let session = manager.get_session(&nonexistent_id).unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn test_update_session() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd).unwrap();

        let message = Message::new(MessageRole::User, "Hello".to_string());

        manager
            .update_session(&session_id, |session| {
                session.add_message(message.clone());
            })
            .unwrap();

        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.context.len(), 1);
        assert_eq!(session.context[0].content, "Hello");
    }

    #[test]
    fn test_update_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = SessionId::new();

        // Should not panic when trying to update a non-existent session
        let result = manager.update_session(&nonexistent_id, |session| {
            session.add_message(Message::new(MessageRole::User, "test".to_string()));
        });

        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_session() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd).unwrap();

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
        let nonexistent_id = SessionId::new();

        let removed = manager.remove_session(&nonexistent_id).unwrap();
        assert!(removed.is_none());
    }

    #[test]
    fn test_list_sessions() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        // Initially empty
        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 0);

        // Create some sessions
        let id1 = manager.create_session(cwd.clone()).unwrap();
        let id2 = manager.create_session(cwd).unwrap();

        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&id1));
        assert!(sessions.contains(&id2));
    }

    #[test]
    fn test_session_count() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        assert_eq!(manager.session_count().unwrap(), 0);

        manager.create_session(cwd.clone()).unwrap();
        assert_eq!(manager.session_count().unwrap(), 1);

        manager.create_session(cwd).unwrap();
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
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd).unwrap();
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

    #[test]
    #[should_panic(expected = "Session working directory must be absolute path")]
    fn test_session_creation_with_relative_path_panics() {
        let session_id = SessionId::new();
        let relative_path = PathBuf::from("./relative/path");
        let _session = Session::new(session_id, relative_path);
    }

    #[test]
    fn test_create_session_with_invalid_working_directory() {
        let manager = SessionManager::new();
        let invalid_path = PathBuf::from("/nonexistent/directory");

        let result = manager.create_session(invalid_path);
        assert!(result.is_err());

        if let Err(crate::AgentError::Session(msg)) = result {
            assert!(msg.contains("Working directory validation failed"));
        } else {
            panic!("Expected Session error with working directory validation message");
        }
    }

    #[test]
    fn test_session_stores_working_directory() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        let session_id = manager.create_session(cwd.clone()).unwrap();
        let session = manager.get_session(&session_id).unwrap().unwrap();

        assert_eq!(session.cwd, cwd);
    }

    #[test]
    fn test_working_directory_validation_during_session_creation() {
        let manager = SessionManager::new();
        let non_absolute_path = PathBuf::from("relative/path");

        let result = manager.create_session(non_absolute_path);
        assert!(result.is_err());

        if let Err(crate::AgentError::Session(msg)) = result {
            assert!(msg.contains("Working directory validation failed"));
            assert!(msg.contains("must be absolute"));
        } else {
            panic!("Expected Session error with absolute path requirement");
        }
    }

    #[test]
    fn test_working_directory_preserved_across_session_operations() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        let session_id = manager.create_session(cwd.clone()).unwrap();

        // Add a message to the session
        manager
            .update_session(&session_id, |session| {
                session.add_message(Message::new(MessageRole::User, "test".to_string()));
            })
            .unwrap();

        // Retrieve session and verify working directory is preserved
        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.cwd, cwd);
        assert_eq!(session.context.len(), 1);
    }

    #[test]
    fn test_different_sessions_can_have_different_working_directories() {
        let manager = SessionManager::new();
        let cwd1 = std::env::current_dir().unwrap();
        let cwd2 = std::env::temp_dir();

        let session_id1 = manager.create_session(cwd1.clone()).unwrap();
        let session_id2 = manager.create_session(cwd2.clone()).unwrap();

        let session1 = manager.get_session(&session_id1).unwrap().unwrap();
        let session2 = manager.get_session(&session_id2).unwrap().unwrap();

        assert_eq!(session1.cwd, cwd1);
        assert_eq!(session2.cwd, cwd2);
        assert_ne!(session1.cwd, session2.cwd);
    }

    #[test]
    fn test_session_serialization_includes_working_directory() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let session = Session::new(session_id, cwd.clone());

        // Test serialization
        let serialized = serde_json::to_string(&session).unwrap();
        assert!(serialized.contains(&cwd.to_string_lossy().to_string()));

        // Test deserialization
        let deserialized: Session = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, session_id);
        assert_eq!(deserialized.cwd, cwd);
    }

    #[cfg(unix)]
    #[test]
    fn test_unix_absolute_path_validation() {
        let manager = SessionManager::new();
        let unix_absolute = PathBuf::from("/tmp");

        let result = manager.create_session(unix_absolute.clone());
        assert!(result.is_ok());

        let session_id = result.unwrap();
        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.cwd, unix_absolute);
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_absolute_path_validation() {
        let manager = SessionManager::new();
        let windows_absolute = PathBuf::from("C:\\temp");

        // This test assumes C:\temp exists on Windows systems
        // In real scenarios, we'd use a guaranteed existing path
        if windows_absolute.exists() {
            let result = manager.create_session(windows_absolute.clone());
            assert!(result.is_ok());

            let session_id = result.unwrap();
            let session = manager.get_session(&session_id).unwrap().unwrap();
            assert_eq!(session.cwd, windows_absolute);
        }
    }

    #[test]
    fn test_working_directory_must_exist() {
        let manager = SessionManager::new();
        let non_existent = PathBuf::from("/this/path/definitely/does/not/exist/nowhere");

        let result = manager.create_session(non_existent);
        assert!(result.is_err());

        if let Err(crate::AgentError::Session(msg)) = result {
            assert!(msg.contains("Working directory validation failed"));
        } else {
            panic!("Expected Session error for non-existent directory");
        }
    }

    #[test]
    fn test_session_has_available_commands_field() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let session = Session::new(session_id, cwd);

        // Session should have an available_commands field
        assert_eq!(session.available_commands.len(), 0);
    }

    #[test]
    fn test_session_update_available_commands() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);

        let commands = vec![
            agent_client_protocol::AvailableCommand {
                name: "create_plan".to_string(),
                description: "Create an execution plan for complex tasks".to_string(),
                input: None,
                meta: None,
            },
            agent_client_protocol::AvailableCommand {
                name: "research_codebase".to_string(),
                description: "Research and analyze the codebase structure".to_string(),
                input: None,
                meta: None,
            },
        ];

        session.update_available_commands(commands.clone());
        assert_eq!(session.available_commands.len(), 2);
        assert_eq!(session.available_commands[0].name, "create_plan");
        assert_eq!(session.available_commands[1].name, "research_codebase");
    }

    #[test]
    fn test_session_detect_available_commands_changes() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);

        let initial_commands = vec![agent_client_protocol::AvailableCommand {
            name: "create_plan".to_string(),
            description: "Create an execution plan for complex tasks".to_string(),
            input: None,
            meta: None,
        }];

        // Set initial commands
        session.update_available_commands(initial_commands.clone());
        assert!(!session.has_available_commands_changed(&initial_commands));

        // Change commands - should detect difference
        let updated_commands = vec![agent_client_protocol::AvailableCommand {
            name: "research_codebase".to_string(),
            description: "Research and analyze the codebase structure".to_string(),
            input: None,
            meta: None,
        }];

        assert!(session.has_available_commands_changed(&updated_commands));
    }

    #[test]
    fn test_session_manager_send_available_commands_update() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd).unwrap();

        let commands = vec![agent_client_protocol::AvailableCommand {
            name: "create_plan".to_string(),
            description: "Create an execution plan for complex tasks".to_string(),
            input: None,
            meta: None,
        }];

        // This should update session and return whether an update was sent
        let update_sent = manager
            .update_available_commands(&session_id, commands)
            .unwrap();
        assert!(update_sent);

        // Same commands again - should not send update
        let commands = vec![agent_client_protocol::AvailableCommand {
            name: "create_plan".to_string(),
            description: "Create an execution plan for complex tasks".to_string(),
            input: None,
            meta: None,
        }];
        let update_sent = manager
            .update_available_commands(&session_id, commands)
            .unwrap();
        assert!(!update_sent);
    }
}
