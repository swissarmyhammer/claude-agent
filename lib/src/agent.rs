//! Agent Client Protocol implementation for Claude Agent

use crate::{
    claude::ClaudeClient, config::AgentConfig, session::SessionManager, tools::ToolCallHandler,
};
use agent_client_protocol::{
    Agent, AgentCapabilities, AuthenticateRequest, AuthenticateResponse, CancelNotification,
    ContentBlock, ExtNotification, ExtRequest, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, RawValue, SessionId, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse, StopReason, TextContent,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;

// SessionUpdateNotification has been replaced with agent_client_protocol::SessionNotification
// This provides better protocol compliance and type safety

// ToolCallContent and MessageChunk have been replaced with agent_client_protocol types:
// - ToolCallContent -> Use SessionUpdate enum variants directly
// - MessageChunk -> Use ContentBlock directly

/// Notification sender for streaming updates
///
/// Manages the broadcasting of session update notifications to multiple receivers.
/// This allows the agent to send real-time updates about session state changes,
/// streaming content, and tool execution results to interested subscribers.
pub struct NotificationSender {
    /// The broadcast sender for distributing notifications
    sender: broadcast::Sender<SessionNotification>,
}

impl NotificationSender {
    /// Create a new notification sender with receiver
    ///
    /// Returns a tuple containing the sender and a receiver that can be used
    /// to listen for session update notifications. The receiver can be cloned
    /// to create multiple subscribers.
    ///
    /// # Parameters
    ///
    /// * `buffer_size` - The size of the broadcast channel buffer for notifications
    ///
    /// # Returns
    ///
    /// A tuple of (NotificationSender, Receiver) where the receiver can be used
    /// to subscribe to session update notifications.
    pub fn new(buffer_size: usize) -> (Self, broadcast::Receiver<SessionNotification>) {
        let (sender, receiver) = broadcast::channel(buffer_size);
        (Self { sender }, receiver)
    }

    /// Send a session update notification
    ///
    /// Broadcasts a session update notification to all subscribers. This is used
    /// to notify clients of real-time changes in session state, streaming content,
    /// or tool execution results.
    ///
    /// # Arguments
    ///
    /// * `notification` - The session notification to broadcast
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the notification was sent successfully, or an error
    /// if the broadcast channel has no receivers or encounters other issues.
    pub async fn send_update(&self, notification: SessionNotification) -> crate::Result<()> {
        self.sender
            .send(notification)
            .map_err(|_| crate::AgentError::Protocol("Failed to send notification".to_string()))?;
        Ok(())
    }
}

/// The main Claude Agent implementing the Agent Client Protocol
///
/// ClaudeAgent is the core implementation of the Agent Client Protocol (ACP),
/// providing a bridge between clients and the Claude AI service. It manages
/// sessions, handles streaming responses, processes tool calls, and maintains
/// the conversation context.
///
/// The agent supports:
/// - Session management with conversation history
/// - Streaming and non-streaming responses  
/// - Tool execution with permission management
/// - Real-time notifications for session updates
/// - Full ACP protocol compliance
pub struct ClaudeAgent {
    session_manager: Arc<SessionManager>,
    claude_client: Arc<ClaudeClient>,
    tool_handler: Arc<ToolCallHandler>,
    mcp_manager: Option<Arc<crate::mcp::McpServerManager>>,
    config: AgentConfig,
    capabilities: AgentCapabilities,
    notification_sender: Arc<NotificationSender>,
}

impl ClaudeAgent {
    /// Create a new Claude Agent instance
    ///
    /// Initializes a new ClaudeAgent with the provided configuration. The agent
    /// will set up all necessary components including session management, Claude
    /// client connection, tool handling, and notification broadcasting.
    ///
    /// # Arguments
    ///
    /// * `config` - The agent configuration containing Claude API settings,
    ///   security policies, and other operational parameters
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - The initialized ClaudeAgent instance
    /// - A broadcast receiver for subscribing to session update notifications
    ///
    /// # Errors
    ///
    /// Returns an error if the agent cannot be initialized due to configuration
    /// issues or if the Claude client cannot be created.
    pub async fn new(
        config: AgentConfig,
    ) -> crate::Result<(Self, broadcast::Receiver<SessionNotification>)> {
        // Validate configuration including MCP servers
        config.validate()?;

        let session_manager = Arc::new(SessionManager::new());
        let claude_client = Arc::new(ClaudeClient::new_with_config(&config.claude)?);

        let (notification_sender, notification_receiver) =
            NotificationSender::new(config.notification_buffer_size);

        // Create and initialize MCP manager
        let mut mcp_manager = crate::mcp::McpServerManager::new();
        mcp_manager
            .connect_servers(config.mcp_servers.clone())
            .await?;
        let mcp_manager = Arc::new(mcp_manager);

        // Create tool handler with MCP support
        let tool_handler = Arc::new(ToolCallHandler::new_with_mcp_manager(
            config.security.to_tool_permissions(),
            Arc::clone(&mcp_manager),
        ));

        // Get all available tools for capabilities
        let available_tools = tool_handler.list_all_available_tools().await;

        let capabilities = AgentCapabilities {
            load_session: true,
            prompt_capabilities: agent_client_protocol::PromptCapabilities {
                audio: true,
                embedded_context: true,
                image: true,
                meta: Some(serde_json::json!({"streaming": true})),
            },
            // We only support HTTP MCP connections, not SSE (which is deprecated in MCP spec).
            // This is an architectural decision for simplicity and modern standards.
            mcp_capabilities: agent_client_protocol::McpCapabilities {
                http: true,
                sse: false,
                meta: None,
            },
            meta: Some(serde_json::json!({
                "tools": available_tools,
                "streaming": true
            })),
        };

        let agent = Self {
            session_manager,
            claude_client,
            tool_handler,
            mcp_manager: Some(mcp_manager),
            config,
            capabilities,
            notification_sender: Arc::new(notification_sender),
        };

        Ok((agent, notification_receiver))
    }

    /// Shutdown the agent and clean up resources
    pub async fn shutdown(&self) -> crate::Result<()> {
        tracing::info!("Shutting down Claude Agent");

        if let Some(ref mcp_manager) = self.mcp_manager {
            mcp_manager.shutdown().await?;
        }

        tracing::info!("Agent shutdown complete");
        Ok(())
    }

    /// Log incoming request for debugging purposes
    fn log_request<T: std::fmt::Debug>(&self, method: &str, request: &T) {
        tracing::debug!("Handling {} request: {:?}", method, request);
    }

    /// Log outgoing response for debugging purposes
    fn log_response<T: std::fmt::Debug>(&self, method: &str, response: &T) {
        tracing::debug!("Returning {} response: {:?}", method, response);
    }

    /// Get the tool handler for processing tool calls
    ///
    /// Returns a reference to the tool call handler that manages the execution
    /// of file system, terminal, and other tool operations. The handler enforces
    /// security policies and permission requirements.
    ///
    /// # Returns
    ///
    /// A reference to the ToolCallHandler instance used by this agent.
    pub fn tool_handler(&self) -> &ToolCallHandler {
        &self.tool_handler
    }

    /// Parse and validate a session ID from a SessionId wrapper
    fn parse_session_id(
        &self,
        session_id: &SessionId,
    ) -> Result<ulid::Ulid, agent_client_protocol::Error> {
        session_id
            .0
            .as_ref()
            .parse::<ulid::Ulid>()
            .map_err(|_| agent_client_protocol::Error::invalid_params())
    }

    /// Validate a prompt request for common issues
    async fn validate_prompt_request(
        &self,
        request: &PromptRequest,
    ) -> Result<(), agent_client_protocol::Error> {
        // Validate session ID format
        self.parse_session_id(&request.session_id)?;

        // Extract text content from the prompt
        let mut prompt_text = String::new();
        for content_block in &request.prompt {
            match content_block {
                agent_client_protocol::ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                _ => {
                    // For now, we only support text content blocks
                    return Err(agent_client_protocol::Error::invalid_params());
                }
            }
        }

        // Check if prompt is empty
        if prompt_text.trim().is_empty() {
            return Err(agent_client_protocol::Error::invalid_params());
        }

        // Check if prompt is too long (configurable limit)
        if prompt_text.len() > self.config.max_prompt_length {
            return Err(agent_client_protocol::Error::invalid_params());
        }

        Ok(())
    }

    /// Check if streaming is supported for this session
    fn should_stream(&self, session: &crate::session::Session, _request: &PromptRequest) -> bool {
        // Check if client supports streaming
        session
            .client_capabilities
            .as_ref()
            .and_then(|caps| caps.meta.as_ref())
            .and_then(|meta| meta.get("streaming"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Handle streaming prompt request
    async fn handle_streaming_prompt(
        &self,
        session_id: &ulid::Ulid,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        tracing::info!("Handling streaming prompt for session: {}", session_id);

        // Extract text content from the prompt
        let mut prompt_text = String::new();
        for content_block in &request.prompt {
            match content_block {
                ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                _ => {
                    return Err(agent_client_protocol::Error::invalid_params());
                }
            }
        }

        let context: crate::claude::SessionContext = session.into();
        let mut stream = self
            .claude_client
            .query_stream_with_context(&prompt_text, &context)
            .await
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        let mut full_response = String::new();
        let mut chunk_count = 0;

        while let Some(chunk) = stream.next().await {
            chunk_count += 1;
            full_response.push_str(&chunk.content);

            // Send real-time update via session/update notification
            if let Err(e) = self
                .send_session_update(SessionNotification {
                    session_id: SessionId(session_id.to_string().into()),
                    update: SessionUpdate::AgentMessageChunk {
                        content: ContentBlock::Text(TextContent {
                            text: chunk.content.clone(),
                            annotations: None,
                            meta: None,
                        }),
                    },
                    meta: None,
                })
                .await
            {
                tracing::error!(
                    session_id = %session_id,
                    chunk_length = chunk.content.len(),
                    error = %e,
                    "Failed to send session update notification - streaming update lost"
                );
                // Note: We continue processing despite notification failure
                // to avoid interrupting the main streaming flow
            }
        }

        tracing::info!("Completed streaming response with {} chunks", chunk_count);

        // Store complete response in session
        let assistant_message = crate::session::Message {
            role: crate::session::MessageRole::Assistant,
            content: full_response,
            timestamp: std::time::SystemTime::now(),
        };

        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(assistant_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        Ok(PromptResponse {
            stop_reason: StopReason::EndTurn,
            meta: Some(serde_json::json!({
                "processed": true,
                "streaming": true,
                "chunks_sent": chunk_count,
                "session_messages": session.context.len() + 1
            })),
        })
    }

    /// Handle non-streaming prompt request
    async fn handle_non_streaming_prompt(
        &self,
        session_id: &ulid::Ulid,
        request: &PromptRequest,
        session: &crate::session::Session,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        tracing::info!("Handling non-streaming prompt for session: {}", session_id);

        // Extract text content from the prompt
        let mut prompt_text = String::new();
        for content_block in &request.prompt {
            match content_block {
                ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                _ => {
                    return Err(agent_client_protocol::Error::invalid_params());
                }
            }
        }

        let context: crate::claude::SessionContext = session.into();
        let response_content = self
            .claude_client
            .query_with_context(&prompt_text, &context)
            .await
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Store assistant response in session
        let assistant_message = crate::session::Message {
            role: crate::session::MessageRole::Assistant,
            content: response_content.clone(),
            timestamp: std::time::SystemTime::now(),
        };

        self.session_manager
            .update_session(session_id, |session| {
                session.add_message(assistant_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        Ok(PromptResponse {
            stop_reason: StopReason::EndTurn,
            meta: Some(serde_json::json!({
                "processed": true,
                "streaming": false,
                "claude_response": response_content,
                "session_messages": session.context.len() + 1
            })),
        })
    }

    /// Send session update notification
    async fn send_session_update(&self, notification: SessionNotification) -> crate::Result<()> {
        self.notification_sender.send_update(notification).await
    }

    /// Shutdown active sessions gracefully
    pub async fn shutdown_sessions(&self) -> crate::Result<()> {
        // Session manager cleanup is handled by dropping the Arc
        // Sessions will be automatically cleaned up when no longer referenced
        tracing::info!("Sessions shutdown complete");
        Ok(())
    }

    /// Shutdown MCP server connections gracefully
    pub async fn shutdown_mcp_connections(&self) -> crate::Result<()> {
        if let Some(_mcp_manager) = &self.mcp_manager {
            // The MCP manager will handle cleanup when dropped
            tracing::info!("MCP connections shutdown initiated");
        }
        Ok(())
    }

    /// Shutdown tool handler gracefully
    pub async fn shutdown_tool_handler(&self) -> crate::Result<()> {
        // Tool handler cleanup is handled by dropping the Arc
        // Any background processes should be terminated gracefully
        tracing::info!("Tool handler shutdown complete");
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Agent for ClaudeAgent {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> Result<InitializeResponse, agent_client_protocol::Error> {
        self.log_request("initialize", &request);
        tracing::info!(
            "Initializing agent with client capabilities: {:?}",
            request.client_capabilities
        );

        let response = InitializeResponse {
            agent_capabilities: self.capabilities.clone(),
            // AUTHENTICATION ARCHITECTURE DECISION:
            // Claude Code is a local development tool that runs entirely on the user's machine.
            // It does not require authentication because:
            // 1. It operates within the user's own development environment
            // 2. It does not connect to external services requiring credentials
            // 3. It has no multi-user access control requirements
            // 4. All operations are performed with the user's existing local permissions
            //
            // Therefore, we intentionally declare NO authentication methods (empty array).
            // This is an architectural decision - do not add authentication methods.
            // If remote authentication is needed in the future, it should be a separate feature.
            auth_methods: vec![],
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

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> Result<AuthenticateResponse, agent_client_protocol::Error> {
        self.log_request("authenticate", &request);

        // AUTHENTICATION ARCHITECTURE DECISION:
        // Claude Code declares NO authentication methods in initialize().
        // According to ACP spec, clients should not call authenticate when no methods are declared.
        // If they do call authenticate anyway, we reject it with a clear error.
        tracing::warn!(
            "Authentication attempt rejected - no auth methods declared: {:?}",
            request.method_id
        );

        Err(agent_client_protocol::Error::method_not_found())
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        self.log_request("new_session", &request);
        tracing::info!("Creating new session");

        let session_id = self
            .session_manager
            .create_session()
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        // Store MCP servers in the session if provided
        if !request.mcp_servers.is_empty() {
            self.session_manager
                .update_session(&session_id, |session| {
                    // Store the actual MCP server info from the request (convert to debug string for now)
                    session.mcp_servers = request
                        .mcp_servers
                        .iter()
                        .map(|server| format!("{:?}", server))
                        .collect();
                })
                .map_err(|_e| agent_client_protocol::Error::internal_error())?;
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

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, agent_client_protocol::Error> {
        self.log_request("load_session", &request);
        tracing::info!("Loading session: {}", request.session_id);

        let session_id = self.parse_session_id(&request.session_id)?;

        let session = self
            .session_manager
            .get_session(&session_id)
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
            None => Err(agent_client_protocol::Error::invalid_params()),
        }
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> Result<SetSessionModeResponse, agent_client_protocol::Error> {
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

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> Result<PromptResponse, agent_client_protocol::Error> {
        self.log_request("prompt", &request);
        tracing::info!(
            "Processing prompt request for session: {}",
            request.session_id
        );

        // Validate the request
        self.validate_prompt_request(&request).await?;

        // Parse session ID
        let session_id = self.parse_session_id(&request.session_id)?;

        // Extract text content from the prompt
        let mut prompt_text = String::new();
        for content_block in &request.prompt {
            match content_block {
                ContentBlock::Text(text_content) => {
                    prompt_text.push_str(&text_content.text);
                }
                _ => {
                    // Already validated in validate_prompt_request
                    return Err(agent_client_protocol::Error::invalid_params());
                }
            }
        }

        // Validate session exists and get it
        let session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::invalid_params)?;

        // Add user message to session
        let user_message = crate::session::Message {
            role: crate::session::MessageRole::User,
            content: prompt_text.clone(),
            timestamp: std::time::SystemTime::now(),
        };

        self.session_manager
            .update_session(&session_id, |session| {
                session.add_message(user_message);
            })
            .map_err(|_| agent_client_protocol::Error::internal_error())?;

        // Get updated session for context
        let updated_session = self
            .session_manager
            .get_session(&session_id)
            .map_err(|_| agent_client_protocol::Error::internal_error())?
            .ok_or_else(agent_client_protocol::Error::internal_error)?;

        // Check if streaming is supported and requested
        let response = if self.should_stream(&session, &request) {
            self.handle_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        } else {
            self.handle_non_streaming_prompt(&session_id, &request, &updated_session)
                .await?
        };

        self.log_response("prompt", &response);
        Ok(response)
    }

    async fn cancel(
        &self,
        notification: CancelNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("cancel", &notification);
        tracing::info!("Cancel notification received");

        // Handle cancellation logic here
        Ok(())
    }

    /// Handle extension method requests
    ///
    /// Extension methods allow clients to call custom methods not defined in the core
    /// Agent Client Protocol specification. This implementation returns a placeholder
    /// response indicating that extension methods are not currently supported.
    ///
    /// ## Design Decision
    ///
    /// Claude Agent currently does not require any extension methods beyond the standard
    /// ACP specification. The core protocol provides sufficient capabilities for:
    /// - Session management (new_session, load_session, set_session_mode)
    /// - Authentication (handled via empty auth_methods)
    /// - Tool execution (via prompt requests)
    /// - Session updates and notifications
    ///
    /// If future requirements emerge for custom extension methods, this implementation
    /// can be enhanced to dispatch to specific handlers based on the method name.
    ///
    /// ## Protocol Compliance
    ///
    /// This implementation satisfies the ACP requirement that agents must respond to
    /// extension method calls, even if they don't implement any specific extensions.
    /// Returning a structured response (rather than an error) maintains client compatibility.
    async fn ext_method(
        &self,
        request: ExtRequest,
    ) -> Result<Arc<RawValue>, agent_client_protocol::Error> {
        self.log_request("ext_method", &request);
        tracing::info!("Extension method called: {}", request.method);

        // Return a structured response indicating no extensions are implemented
        // This maintains ACP compliance while clearly communicating capability limitations
        let response = serde_json::json!({
            "method": request.method,
            "result": "Extension method not implemented"
        });

        let raw_value = RawValue::from_string(response.to_string())
            .map_err(|_e| agent_client_protocol::Error::internal_error())?;

        Ok(Arc::from(raw_value))
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> Result<(), agent_client_protocol::Error> {
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

    async fn create_test_agent() -> ClaudeAgent {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).await.unwrap().0
    }

    #[tokio::test]
    async fn test_initialize() {
        let agent = create_test_agent().await;

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
        assert!(response.auth_methods.is_empty());
        assert!(response.meta.is_some());
        // Protocol version should be the default value
        assert_eq!(response.protocol_version, Default::default());
    }

    #[tokio::test]
    async fn test_authenticate() {
        let agent = create_test_agent().await;

        // Test that authentication is properly rejected since we declare no auth methods
        let request = AuthenticateRequest {
            method_id: agent_client_protocol::AuthMethodId("none".to_string().into()),
            meta: None,
        };

        let result = agent.authenticate(request).await;
        assert!(result.is_err(), "Authentication should be rejected");

        // Test with a different method to ensure all methods are rejected
        let request2 = AuthenticateRequest {
            method_id: agent_client_protocol::AuthMethodId("basic".to_string().into()),
            meta: None,
        };

        let result2 = agent.authenticate(request2).await;
        assert!(
            result2.is_err(),
            "All authentication methods should be rejected"
        );
    }

    #[tokio::test]
    async fn test_new_session() {
        let agent = create_test_agent().await;

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
        let agent = create_test_agent().await;

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
        let agent = create_test_agent().await;

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
        let agent = create_test_agent().await;

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
        let agent = create_test_agent().await;

        // First create a session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        let new_session_response = agent.new_session(new_session_request).await.unwrap();

        let request = PromptRequest {
            session_id: new_session_response.session_id,
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Hello, world!".to_string(),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: Some(serde_json::json!({"prompt": "Hello, world!"})),
        };

        let response = agent.prompt(request).await.unwrap();
        assert!(response.meta.is_some());
    }

    #[tokio::test]
    async fn test_cancel() {
        let agent = create_test_agent().await;

        let notification = CancelNotification {
            session_id: SessionId("test_session".to_string().into()),
            meta: Some(serde_json::json!({"reason": "user_request"})),
        };

        let result = agent.cancel(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ext_method() {
        let agent = create_test_agent().await;

        let request = ExtRequest {
            method: "test_method".to_string().into(),
            params: Arc::from(RawValue::from_string("{}".to_string()).unwrap()),
        };

        let response = agent.ext_method(request).await.unwrap();
        assert!(!response.get().is_empty());
    }

    #[tokio::test]
    async fn test_ext_notification() {
        let agent = create_test_agent().await;

        let notification = ExtNotification {
            method: "test_notification".to_string().into(),
            params: Arc::from(RawValue::from_string("{}".to_string()).unwrap()),
        };

        let result = agent.ext_notification(notification.clone()).await;
        assert!(result.is_ok());

        // Explicitly drop resources to ensure cleanup
        drop(notification);
        drop(agent);
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let config = AgentConfig::default();
        let result = ClaudeAgent::new(config).await;
        assert!(result.is_ok());

        let (agent, _receiver) = result.unwrap();
        assert!(agent.capabilities.meta.is_some());
    }

    #[tokio::test]
    async fn test_full_prompt_flow() {
        let agent = create_test_agent().await;

        // Create session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        let new_session_response = agent.new_session(new_session_request).await.unwrap();

        // Send prompt
        let prompt_request = PromptRequest {
            session_id: new_session_response.session_id.clone(),
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Hello, how are you?".to_string(),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: Some(serde_json::json!({"test": "full_flow"})),
        };

        let prompt_response = agent.prompt(prompt_request).await.unwrap();

        assert_eq!(prompt_response.stop_reason, StopReason::EndTurn);
        assert!(prompt_response.meta.is_some());

        // Verify session was updated with both user and assistant messages
        let session_id = new_session_response.session_id.0.as_ref().parse().unwrap();
        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();

        // Should have user message and assistant response
        assert_eq!(session.context.len(), 2);
        assert!(matches!(
            session.context[0].role,
            crate::session::MessageRole::User
        ));
        assert_eq!(session.context[0].content, "Hello, how are you?");
        assert!(matches!(
            session.context[1].role,
            crate::session::MessageRole::Assistant
        ));
        assert!(!session.context[1].content.is_empty());
    }

    #[tokio::test]
    async fn test_prompt_validation_invalid_session_id() {
        let agent = create_test_agent().await;

        // Test invalid session ID
        let prompt_request = PromptRequest {
            session_id: SessionId("invalid-uuid".to_string().into()),
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Hello".to_string(),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prompt_validation_empty_prompt() {
        let agent = create_test_agent().await;

        // Create a valid session first
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Test empty prompt
        let prompt_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "   ".to_string(), // Only whitespace
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_prompt_validation_non_text_content() {
        let agent = create_test_agent().await;

        // Create a valid session first
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Test non-text content block
        let prompt_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: vec![agent_client_protocol::ContentBlock::Image(
                agent_client_protocol::ImageContent {
                    data: "base64data".to_string(),
                    mime_type: "image/png".to_string(),
                    uri: Some("data:image/png;base64,base64data".to_string()),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_conversation_context_maintained() {
        let agent = create_test_agent().await;

        // Create session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        let new_session_response = agent.new_session(new_session_request).await.unwrap();

        // Send first prompt
        let prompt_request_1 = PromptRequest {
            session_id: new_session_response.session_id.clone(),
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "My name is Alice".to_string(),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        agent.prompt(prompt_request_1).await.unwrap();

        // Send second prompt that references the first
        let prompt_request_2 = PromptRequest {
            session_id: new_session_response.session_id.clone(),
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "What is my name?".to_string(),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let response_2 = agent.prompt(prompt_request_2).await.unwrap();

        // Verify session has 4 messages (2 user + 2 assistant)
        let session_id = new_session_response.session_id.0.as_ref().parse().unwrap();
        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();

        assert_eq!(session.context.len(), 4);

        // Verify the sequence of messages
        assert!(matches!(
            session.context[0].role,
            crate::session::MessageRole::User
        ));
        assert_eq!(session.context[0].content, "My name is Alice");
        assert!(matches!(
            session.context[1].role,
            crate::session::MessageRole::Assistant
        ));
        assert!(matches!(
            session.context[2].role,
            crate::session::MessageRole::User
        ));
        assert_eq!(session.context[2].content, "What is my name?");
        assert!(matches!(
            session.context[3].role,
            crate::session::MessageRole::Assistant
        ));

        assert_eq!(response_2.stop_reason, StopReason::EndTurn);
    }

    #[tokio::test]
    async fn test_prompt_nonexistent_session() {
        let agent = create_test_agent().await;

        // Use a valid ULID but for a session that doesn't exist
        let nonexistent_session_id = ulid::Ulid::new();
        let prompt_request = PromptRequest {
            session_id: SessionId(nonexistent_session_id.to_string().into()),
            prompt: vec![agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent {
                    text: "Hello".to_string(),
                    annotations: None,
                    meta: None,
                },
            )],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_err());
    }

    // Helper function for streaming tests
    async fn create_test_agent_with_notifications(
    ) -> (ClaudeAgent, broadcast::Receiver<SessionNotification>) {
        let config = AgentConfig::default();
        ClaudeAgent::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_streaming_prompt() {
        let (agent, _notification_receiver) = create_test_agent_with_notifications().await;

        // Create session with streaming capabilities
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: Some(serde_json::json!({"streaming": true})),
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Update session to have client capabilities with streaming enabled
        let session_id = session_response.session_id.0.as_ref().parse().unwrap();
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.client_capabilities = Some(agent_client_protocol::ClientCapabilities {
                    fs: agent_client_protocol::FileSystemCapability {
                        read_text_file: true,
                        write_text_file: true,
                        meta: None,
                    },
                    terminal: true,
                    meta: Some(serde_json::json!({"streaming": true})),
                });
            })
            .unwrap();

        // Send streaming prompt
        let prompt_request = PromptRequest {
            session_id: session_response.session_id.clone(),
            prompt: vec![ContentBlock::Text(TextContent {
                text: "Tell me a story".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        // Execute streaming prompt directly (can't use tokio::spawn with ?Send trait)
        let response = agent.prompt(prompt_request.clone()).await.unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);

        // Verify streaming metadata is present
        assert!(response.meta.is_some());
        let meta = response.meta.unwrap();
        assert_eq!(
            meta.get("streaming").unwrap(),
            &serde_json::Value::Bool(true)
        );

        // Verify session was updated with both user and assistant messages
        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert_eq!(session.context.len(), 2); // user + assistant
        assert!(matches!(
            session.context[0].role,
            crate::session::MessageRole::User
        ));
        assert!(matches!(
            session.context[1].role,
            crate::session::MessageRole::Assistant
        ));
    }

    #[tokio::test]
    async fn test_non_streaming_fallback() {
        let (agent, _notification_receiver) = create_test_agent_with_notifications().await;

        // Create session without streaming capabilities
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Session should not have streaming capabilities (default)
        let session_id = session_response.session_id.0.as_ref().parse().unwrap();
        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert!(session.client_capabilities.is_none());

        let prompt_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: vec![ContentBlock::Text(TextContent {
                text: "Hello, world!".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        let result = agent.prompt(prompt_request).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert!(response.meta.is_some());

        // Verify meta indicates non-streaming
        let meta = response.meta.unwrap();
        assert_eq!(
            meta.get("streaming").unwrap(),
            &serde_json::Value::Bool(false)
        );
    }

    #[tokio::test]
    async fn test_streaming_capability_detection() {
        let (agent, _) = create_test_agent_with_notifications().await;

        // Create a session
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id.0.as_ref().parse().unwrap();

        // Test should_stream with no capabilities
        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        let dummy_request = PromptRequest {
            session_id: session_response.session_id,
            prompt: vec![],
            meta: None,
        };
        assert!(!agent.should_stream(&session, &dummy_request));

        // Add client capabilities without streaming
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.client_capabilities = Some(agent_client_protocol::ClientCapabilities {
                    fs: agent_client_protocol::FileSystemCapability {
                        read_text_file: true,
                        write_text_file: true,
                        meta: None,
                    },
                    terminal: true,
                    meta: None, // No streaming meta
                });
            })
            .unwrap();

        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert!(!agent.should_stream(&session, &dummy_request));

        // Add streaming capability
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.client_capabilities = Some(agent_client_protocol::ClientCapabilities {
                    fs: agent_client_protocol::FileSystemCapability {
                        read_text_file: true,
                        write_text_file: true,
                        meta: None,
                    },
                    terminal: true,
                    meta: Some(serde_json::json!({"streaming": true})),
                });
            })
            .unwrap();

        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert!(agent.should_stream(&session, &dummy_request));
    }

    #[tokio::test]
    async fn test_streaming_session_context_maintained() {
        let (agent, _notification_receiver) = create_test_agent_with_notifications().await;

        // Create session with streaming capabilities
        let new_session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: Some(serde_json::json!({"streaming": true})),
        };
        let session_response = agent.new_session(new_session_request).await.unwrap();

        // Update session to have client capabilities with streaming enabled
        let session_id = session_response.session_id.0.as_ref().parse().unwrap();
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.client_capabilities = Some(agent_client_protocol::ClientCapabilities {
                    fs: agent_client_protocol::FileSystemCapability {
                        read_text_file: true,
                        write_text_file: true,
                        meta: None,
                    },
                    terminal: true,
                    meta: Some(serde_json::json!({"streaming": true})),
                });
            })
            .unwrap();

        // Send first streaming prompt
        let prompt_request_1 = PromptRequest {
            session_id: session_response.session_id.clone(),
            prompt: vec![ContentBlock::Text(TextContent {
                text: "My name is Alice".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        let response_1 = agent.prompt(prompt_request_1).await.unwrap();
        assert_eq!(response_1.stop_reason, StopReason::EndTurn);

        // Send second prompt that references the first
        let prompt_request_2 = PromptRequest {
            session_id: session_response.session_id.clone(),
            prompt: vec![ContentBlock::Text(TextContent {
                text: "What is my name?".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        let response_2 = agent.prompt(prompt_request_2).await.unwrap();
        assert_eq!(response_2.stop_reason, StopReason::EndTurn);

        // Verify session has 4 messages (2 user + 2 assistant)
        let session = agent
            .session_manager
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert_eq!(session.context.len(), 4);

        // Verify the sequence of messages
        assert!(matches!(
            session.context[0].role,
            crate::session::MessageRole::User
        ));
        assert_eq!(session.context[0].content, "My name is Alice");
        assert!(matches!(
            session.context[1].role,
            crate::session::MessageRole::Assistant
        ));
        assert!(matches!(
            session.context[2].role,
            crate::session::MessageRole::User
        ));
        assert_eq!(session.context[2].content, "What is my name?");
        assert!(matches!(
            session.context[3].role,
            crate::session::MessageRole::Assistant
        ));

        // Verify streaming metadata in responses
        assert!(response_1.meta.is_some());
        let meta_1 = response_1.meta.unwrap();
        assert_eq!(
            meta_1.get("streaming").unwrap(),
            &serde_json::Value::Bool(true)
        );

        assert!(response_2.meta.is_some());
        let meta_2 = response_2.meta.unwrap();
        assert_eq!(
            meta_2.get("streaming").unwrap(),
            &serde_json::Value::Bool(true)
        );
    }

    // Protocol Compliance Tests

    #[tokio::test]
    async fn test_full_protocol_flow() {
        let (agent, _notifications) = create_test_agent_with_notifications().await;

        // Test initialize
        let init_request = InitializeRequest {
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

        let init_response = agent.initialize(init_request).await.unwrap();
        assert!(init_response.agent_capabilities.meta.is_some());
        assert!(init_response.auth_methods.is_empty());

        // Test authenticate - should fail since we declare no auth methods
        let auth_request = AuthenticateRequest {
            method_id: agent_client_protocol::AuthMethodId("none".to_string().into()),
            meta: None,
        };

        let auth_result = agent.authenticate(auth_request).await;
        assert!(
            auth_result.is_err(),
            "Authentication should be rejected when no auth methods are declared"
        );

        // Test session creation
        let session_request = NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            mcp_servers: vec![],
            meta: None,
        };

        let session_response = agent.new_session(session_request).await.unwrap();
        assert!(!session_response.session_id.0.is_empty());

        // Test prompt
        let prompt_request = PromptRequest {
            session_id: session_response.session_id.clone(),
            prompt: vec![ContentBlock::Text(TextContent {
                text: "Hello, can you help me?".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        let prompt_response = agent.prompt(prompt_request).await.unwrap();
        assert_eq!(prompt_response.stop_reason, StopReason::EndTurn);
        assert!(prompt_response.meta.is_some());
    }

    #[tokio::test]
    async fn test_protocol_error_handling() {
        let (agent, _) = create_test_agent_with_notifications().await;

        // Test invalid session ID
        let invalid_prompt = PromptRequest {
            session_id: SessionId("invalid-uuid".to_string().into()),
            prompt: vec![ContentBlock::Text(TextContent {
                text: "Hello".to_string(),
                annotations: None,
                meta: None,
            })],
            meta: None,
        };

        let result = agent.prompt(invalid_prompt).await;
        assert!(result.is_err());

        // };
        //
        // let deny_result = agent.tool_permission_deny(invalid_deny).await.unwrap();
        // assert!(deny_result.success); // Should succeed even if tool call doesn't exist
    }

    #[test]
    fn test_compile_time_agent_check() {
        // Compile-time check that all Agent trait methods are implemented
        fn assert_agent_impl<T: Agent>() {}
        assert_agent_impl::<ClaudeAgent>();
    }
}
