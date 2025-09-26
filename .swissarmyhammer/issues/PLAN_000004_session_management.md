# Session Management System

Refer to plan.md

## Goal
Build a thread-safe session management system to track conversation contexts and state.

## Tasks

### 1. Session Types (`lib/src/session.rs`)

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;
use agent_client_protocol::ClientCapabilities;

pub type SessionId = Uuid;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: SessionId,
    pub created_at: std::time::SystemTime,
    pub last_accessed: std::time::SystemTime,
    pub context: Vec<Message>,
    pub client_capabilities: Option<ClientCapabilities>,
    pub mcp_servers: Vec<String>, // Server names
}

impl Session {
    pub fn new(id: SessionId) -> Self {
        let now = std::time::SystemTime::now();
        Self {
            id,
            created_at: now,
            last_accessed: now,
            context: Vec::new(),
            client_capabilities: None,
            mcp_servers: Vec::new(),
        }
    }
    
    pub fn add_message(&mut self, message: Message) {
        self.context.push(message);
        self.last_accessed = std::time::SystemTime::now();
    }
    
    pub fn update_access_time(&mut self) {
        self.last_accessed = std::time::SystemTime::now();
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}
```

### 2. Session Manager

```rust
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    cleanup_interval: std::time::Duration,
    max_session_age: std::time::Duration,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval: std::time::Duration::from_secs(300), // 5 minutes
            max_session_age: std::time::Duration::from_secs(3600), // 1 hour
        }
    }
    
    pub fn create_session(&self) -> crate::Result<SessionId> {
        let session_id = Uuid::new_v4();
        let session = Session::new(session_id);
        
        let mut sessions = self.sessions.write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;
        
        sessions.insert(session_id, session);
        Ok(session_id)
    }
    
    pub fn get_session(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        let sessions = self.sessions.read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;
        
        Ok(sessions.get(session_id).cloned())
    }
    
    pub fn update_session<F>(&self, session_id: &SessionId, updater: F) -> crate::Result<()>
    where
        F: FnOnce(&mut Session),
    {
        let mut sessions = self.sessions.write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;
        
        if let Some(session) = sessions.get_mut(session_id) {
            updater(session);
            session.update_access_time();
        }
        
        Ok(())
    }
    
    pub fn remove_session(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        let mut sessions = self.sessions.write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;
        
        Ok(sessions.remove(session_id))
    }
    
    pub fn list_sessions(&self) -> crate::Result<Vec<SessionId>> {
        let sessions = self.sessions.read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;
        
        Ok(sessions.keys().cloned().collect())
    }
}
```

### 3. Session Cleanup

```rust
impl SessionManager {
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let manager = Arc::clone(&self);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(manager.cleanup_interval);
            
            loop {
                interval.tick().await;
                if let Err(e) = manager.cleanup_expired_sessions().await {
                    tracing::error!("Session cleanup failed: {}", e);
                }
            }
        });
    }
    
    async fn cleanup_expired_sessions(&self) -> crate::Result<()> {
        let now = std::time::SystemTime::now();
        let mut expired_sessions = Vec::new();
        
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
        
        for session_id in expired_sessions {
            tracing::info!("Cleaning up expired session: {}", session_id);
            self.remove_session(&session_id)?;
        }
        
        Ok(())
    }
}
```

### 4. Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_creation() {
        let session_id = Uuid::new_v4();
        let session = Session::new(session_id);
        
        assert_eq!(session.id, session_id);
        assert!(session.context.is_empty());
    }
    
    #[test]
    fn test_session_manager() {
        let manager = SessionManager::new();
        
        let session_id = manager.create_session().unwrap();
        let session = manager.get_session(&session_id).unwrap();
        
        assert!(session.is_some());
        assert_eq!(session.unwrap().id, session_id);
    }
    
    #[test]
    fn test_session_update() {
        let manager = SessionManager::new();
        let session_id = manager.create_session().unwrap();
        
        let message = Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
            timestamp: std::time::SystemTime::now(),
        };
        
        manager.update_session(&session_id, |session| {
            session.add_message(message.clone());
        }).unwrap();
        
        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.context.len(), 1);
    }
    
    #[tokio::test]
    async fn test_session_cleanup() {
        // Test cleanup functionality
    }
}
```

## Files Created
- `lib/src/session.rs` - Session management system
- Update `lib/src/lib.rs` to export session module

## Acceptance Criteria
- Sessions can be created with unique UUIDs
- Thread-safe access to session data
- Messages can be added to session context
- Session cleanup removes expired sessions
- Unit tests pass for all functionality
- `cargo build` and `cargo test` succeed