//! Agent Client Protocol implementation for Claude Agent

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, InitializeRequest, InitializeResponse,
    NewSessionRequest, NewSessionResponse, LoadSessionRequest, LoadSessionResponse,
    SetSessionModeRequest, SetSessionModeResponse, PromptRequest, PromptResponse,
    CancelNotification, ExtRequest, ExtNotification, RawValue, AgentCapabilities,
    SessionId, StopReason,
};
use std::sync::Arc;
use crate::{session::SessionManager, claude::ClaudeClient, config::AgentConfig};

/// The main Claude Agent implementing the Agent Client Protocol
pub struct ClaudeAgent {
    session_manager: Arc<SessionManager>,
    claude_client: Arc<ClaudeClient>,
    config: AgentConfig,
    capabilities: AgentCapabilities,
}

impl ClaudeAgent {
    /// Create a new Claude Agent instance
    pub fn new(config: AgentConfig) -> crate::Result<Self> {
        let session_manager = Arc::new(SessionManager::new());
        let claude_client = Arc::new(ClaudeClient::new_with_config(&config.claude)?);
        
        let capabilities = AgentCapabilities {
            load_session: true,
            prompt_capabilities: agent_client_protocol::PromptCapabilities {
                audio: false,
                embedded_context: false,
                image: false,
                meta: Some(serde_json::json!({"streaming": true})),
            },
            mcp_capabilities: agent_client_protocol::McpCapabilities {
                http: false,
                sse: false,
                meta: None,
            },
            meta: Some(serde_json::json!({
                "tools": [
                    "fs_read",
                    "fs_write", 
                    "terminal_create",
                    "terminal_write"
                ]
            })),
        };
        
        Ok(Self {
            session_manager,
            claude_client,
            config,
            capabilities,
        })
    }

    /// Log incoming request for debugging purposes
    fn log_request<T: std::fmt::Debug>(&self, method: &str, request: &T) {
        tracing::debug!("Handling {} request: {:?}", method, request);
    }
    
    /// Log outgoing response for debugging purposes
    fn log_response<T: std::fmt::Debug>(&self, method: &str, response: &T) {
        tracing::debug!("Returning {} response: {:?}", method, response);
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for ClaudeAgent {
    async fn initialize(&self, request: InitializeRequest) -> Result<InitializeResponse, agent_client_protocol::Error> {
        self.log_request("initialize", &request);
        tracing::info!("Initializing agent with client capabilities: {:?}", request.client_capabilities);
        
        let response = InitializeResponse {
            agent_capabilities: self.capabilities.clone(),
            auth_methods: vec![agent_client_protocol::AuthMethod { 
                id: agent_client_protocol::AuthMethodId("none".to_string().into()),
                name: "No Authentication".to_string(),
                description: Some("No authentication required".to_string()),
                meta: None,
            }],
            protocol_version: Default::default(),
            meta: Some(serde_json::json!({
                "agent_name": "Claude Agent",
                "version": env!("CARGO_PKG_VERSION"),
                "protocol_supported": "1.0.0"
            })),
        };

        self.log_response("initialize", &response);
        Ok(response)
    }

    async fn authenticate(&self, request: AuthenticateRequest) -> Result<AuthenticateResponse, agent_client_protocol::Error> {
        self.log_request("authenticate", &request);
        tracing::info!("Authentication requested with method: {:?}", request.method_id);
        
        // For now, always succeed with authentication
        let response = match request.method_id.0.as_ref() {
            "none" => {
                tracing::info!("No authentication required");
                AuthenticateResponse {
                    meta: Some(serde_json::json!({
                        "success": true,
                        "message": "No authentication required"
                    })),
                }
            }
            _ => {
                // For any other method, we'll accept it for now
                tracing::info!("Accepting authentication method: {:?}", request.method_id);
                AuthenticateResponse {
                    meta: Some(serde_json::json!({
                        "success": true,
                        "method": request.method_id.0
                    })),
                }
            }
        };

        self.log_response("authenticate", &response);
        Ok(response)
    }

    async fn new_session(&self, request: NewSessionRequest) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        self.log_request("new_session", &request);
        tracing::info!("Creating new session");
        
        let session_id = self.session_manager.create_session()
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        
        // Store MCP servers in the session if provided  
        if !request.mcp_servers.is_empty() {
            self.session_manager.update_session(&session_id, |session| {
                // Store MCP server names from request - for now just placeholder
                session.mcp_servers = vec!["mcp_server".to_string()];
            }).map_err(|_e| agent_client_protocol::Error::internal_error())?;
        }
        
        tracing::info!("Created session: {}", session_id);
        
        let response = NewSessionResponse {
            session_id: SessionId(session_id.to_string().into()),
            modes: None, // No specific modes for now
            meta: Some(serde_json::json!({
                "created_at": chrono::Utc::now().to_rfc3339()
            })),
        };

        self.log_response("new_session", &response);
        Ok(response)
    }

    async fn load_session(&self, request: LoadSessionRequest) -> Result<LoadSessionResponse, agent_client_protocol::Error> {
        self.log_request("load_session", &request);
        tracing::info!("Loading session: {}", request.session_id);
        
        let session_id = request.session_id.0.as_ref().parse()
            .map_err(|_e| agent_client_protocol::Error::invalid_params())?;
        
        let session = self.session_manager.get_session(&session_id)
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        
        match session {
            Some(session) => {
                tracing::info!("Loaded session: {}", session_id);
                let response = LoadSessionResponse {
                    modes: None, // No specific session modes for now
                    meta: Some(serde_json::json!({
                        "session_id": session.id.to_string(),
                        "created_at": session.created_at.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default().as_secs(),
                        "message_count": session.context.len()
                    })),
                };
                self.log_response("load_session", &response);
                Ok(response)
            }
            None => {
                Err(agent_client_protocol::Error::invalid_params())
            }
        }
    }

    async fn set_session_mode(&self, request: SetSessionModeRequest) -> Result<SetSessionModeResponse, agent_client_protocol::Error> {
        self.log_request("set_session_mode", &request);
        
        // For now, accept any session mode
        let response = SetSessionModeResponse {
            meta: Some(serde_json::json!({
                "mode_set": true,
                "message": "Session mode updated"
            })),
        };

        self.log_response("set_session_mode", &response);
        Ok(response)
    }

    async fn prompt(&self, request: PromptRequest) -> Result<PromptResponse, agent_client_protocol::Error> {
        self.log_request("prompt", &request);
        tracing::info!("Processing prompt request for session: {}", request.session_id);
        
        // Extract text content from the prompt
        let mut prompt_text = String::new();
        for content_block in &request.prompt {
            match content_block {
                agent_client_protocol::ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                    prompt_text.push('\n');
                }
                // For now, we only handle text content blocks
                _ => {
                    tracing::warn!("Non-text content blocks not yet supported");
                }
            }
        }
        
        if prompt_text.trim().is_empty() {
            return Err(agent_client_protocol::Error::invalid_params());
        }
        
        // Use the Claude client to process the prompt
        let session_id_str = request.session_id.0.as_ref();
        match self.claude_client.query(&prompt_text, session_id_str).await {
            Ok(claude_response) => {
                tracing::info!("Successfully processed prompt for session: {}", session_id_str);
                let response = PromptResponse {
                    stop_reason: StopReason::EndTurn,
                    meta: Some(serde_json::json!({
                        "processed": true,
                        "response_type": "text",
                        "claude_response": claude_response,
                        "model": self.config.claude.model
                    })),
                };
                self.log_response("prompt", &response);
                Ok(response)
            }
            Err(e) => {
                tracing::error!("Claude client error: {:?}", e);
                Err(agent_client_protocol::Error::internal_error())
            }
        }
    }

    async fn cancel(&self, notification: CancelNotification) -> Result<(), agent_client_protocol::Error> {
        self.log_request("cancel", &notification);
        tracing::info!("Cancel notification received");
        
        // Handle cancellation logic here
        Ok(())
    }

    async fn ext_method(&self, request: ExtRequest) -> Result<Arc<RawValue>, agent_client_protocol::Error> {
        self.log_request("ext_method", &request);
        tracing::info!("Extension method called: {}", request.method);
        
        // Return a placeholder response
        let response = serde_json::json!({
            "method": request.method,
            "result": "Extension method not implemented"
        });
        
        let raw_value = RawValue::from_string(response.to_string())
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;
        
        Ok(Arc::from(raw_value))
    }

    async fn ext_notification(&self, notification: ExtNotification) -> Result<(), agent_client_protocol::Error> {
        self.log_request("ext_notification", &notification);
        tracing::info!("Extension notification received: {}", notification.method);
        
        // Handle extension notifications
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Import specific types as needed
    use std::sync::Arc;
    
    fn create_test_agent() -> ClaudeAgent {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).unwrap()
    }
    
    #[tokio::test]
    async fn test_initialize() {
        let agent = create_test_agent();
        
        let request = InitializeRequest {
            client_capabilities: agent_client_protocol::ClientCapabilities {
                fs: agent_client_protocol::FileSystemCapability { 
                    read_text_file: true, 
                    write_text_file: true,
                    meta: None,
                },
                terminal: true,
                meta: Some(serde_json::json!({"streaming": true})),
            },
            protocol_version: Default::default(),
            meta: None,
        };
        
        let response = agent.initialize(request).await.unwrap();
        
        assert!(response.agent_capabilities.meta.is_some());
        assert!(!response.auth_methods.is_empty());
        assert!(response.meta.is_some());
        // Protocol version should be the default value
        assert_eq!(response.protocol_version, Default::default());
    }
    
    #[tokio::test]
    async fn test_authenticate() {
        let agent = create_test_agent();
        
        let request = AuthenticateRequest {
            method_id: agent_client_protocol::AuthMethodId("none".to_string().into()),
            meta: None,
        };
        
        let response = agent.authenticate(request).await.unwrap();
        assert!(response.meta.is_some());
    }
    
    #[tokio::test]
    async fn test_new_session() {
        let agent = create_test_agent();
        
        let request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: Some(serde_json::json!({"test": true})),
        };
        
        let response = agent.new_session(request).await.unwrap();
        assert!(!response.session_id.0.is_empty());
        assert!(response.meta.is_some());
        
        // Verify the session was actually created
        let session_id = response.session_id.0.parse().unwrap();
        let session = agent.session_manager.get_session(&session_id).unwrap();
        assert!(session.is_some());
    }

    #[tokio::test]
    async fn test_load_session() {
        let agent = create_test_agent();
        
        // First create a session
        let new_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: Some(serde_json::json!({"test": true})),
        };
        let new_response = agent.new_session(new_request).await.unwrap();
        
        // Now load it
        let load_request = LoadSessionRequest {
            session_id: new_response.session_id,
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        
        let load_response = agent.load_session(load_request).await.unwrap();
        assert!(load_response.meta.is_some());
    }

    #[tokio::test]
    async fn test_load_nonexistent_session() {
        let agent = create_test_agent();
        
        let request = LoadSessionRequest {
            session_id: SessionId("01234567890123456789012345".to_string().into()), // Invalid ULID
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        
        let result = agent.load_session(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_session_mode() {
        let agent = create_test_agent();
        
        let request = SetSessionModeRequest {
            session_id: SessionId("test_session".to_string().into()),
            mode_id: agent_client_protocol::SessionModeId("interactive".to_string().into()),
            meta: Some(serde_json::json!({"mode": "interactive"})),
        };
        
        let response = agent.set_session_mode(request).await.unwrap();
        assert!(response.meta.is_some());
    }

    #[tokio::test]
    async fn test_prompt() {
        let agent = create_test_agent();
        
        let request = PromptRequest {
            session_id: SessionId("test_session".to_string().into()),
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent { 
                    text: "Hello, world!".to_string(),
                    annotations: None,
                    meta: None,
                }
            )],
            meta: Some(serde_json::json!({"prompt": "Hello, world!"})),
        };
        
        let response = agent.prompt(request).await.unwrap();
        assert!(response.meta.is_some());
    }

    #[tokio::test]
    async fn test_cancel() {
        let agent = create_test_agent();
        
        let notification = CancelNotification {
            session_id: SessionId("test_session".to_string().into()),
            meta: Some(serde_json::json!({"reason": "user_request"})),
        };
        
        let result = agent.cancel(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ext_method() {
        let agent = create_test_agent();
        
        let request = ExtRequest {
            method: "test_method".to_string().into(),
            params: Arc::from(RawValue::from_string("{}".to_string()).unwrap()),
        };
        
        let response = agent.ext_method(request).await.unwrap();
        assert!(!response.get().is_empty());
    }

    #[tokio::test]
    async fn test_ext_notification() {
        let agent = create_test_agent();
        
        let notification = ExtNotification {
            method: "test_notification".to_string().into(),
            params: Arc::from(RawValue::from_string("{}".to_string()).unwrap()),
        };
        
        let result = agent.ext_notification(notification).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_agent_creation() {
        let config = AgentConfig::default();
        let agent = ClaudeAgent::new(config);
        assert!(agent.is_ok());
        
        let agent = agent.unwrap();
        assert!(agent.capabilities.meta.is_some());
    }
}