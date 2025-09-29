//! Terminal session management for ACP compliance
//!
//! This module provides comprehensive terminal session management following
//! the Agent Client Protocol (ACP) specification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

/// Manages terminal sessions for command execution
#[derive(Debug, Clone)]
pub struct TerminalManager {
    pub terminals: Arc<RwLock<HashMap<String, TerminalSession>>>,
}

/// Represents a terminal session with working directory and environment
#[derive(Debug)]
pub struct TerminalSession {
    pub process: Option<Child>,
    pub working_dir: std::path::PathBuf,
    pub environment: HashMap<String, String>,
    // ACP-compliant fields for terminal/create method
    pub command: Option<String>,
    pub args: Vec<String>,
    pub session_id: Option<String>,
    pub output_byte_limit: u64,
    pub output_buffer: Vec<u8>,
    pub buffer_truncated: bool,
}

/// ACP-compliant request parameters for terminal/create method
///
/// This struct defines all the parameters needed to create a new terminal session
/// following the Anthropic Computer Protocol (ACP) specification.
#[derive(Debug, Deserialize)]
pub struct TerminalCreateParams {
    /// Session identifier that must exist and be a valid ULID format
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Command to execute in the terminal (e.g., "bash", "python", "echo")
    pub command: String,
    /// Optional command line arguments as a vector of strings
    pub args: Option<Vec<String>>,
    /// Optional environment variables to set for the terminal session
    pub env: Option<Vec<EnvVariable>>,
    /// Optional working directory path (must be absolute if provided)
    pub cwd: Option<String>,
    /// Optional byte limit for terminal output buffering (defaults to system limit)
    #[serde(rename = "outputByteLimit")]
    pub output_byte_limit: Option<u64>,
}

/// Environment variable specification for terminal creation
///
/// Represents a single environment variable to be set in the terminal session.
/// Environment variables override system defaults when names conflict.
#[derive(Debug, Deserialize)]
pub struct EnvVariable {
    /// Environment variable name (cannot be empty)
    pub name: String,
    /// Environment variable value
    pub value: String,
}

/// ACP-compliant response for terminal/create method
///
/// Returns the unique identifier for the newly created terminal session.
/// This terminal ID can be used for subsequent terminal operations.
#[derive(Debug, Serialize)]
pub struct TerminalCreateResponse {
    /// Unique terminal identifier (ULID format)
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

impl TerminalManager {
    /// Create a new terminal manager
    pub fn new() -> Self {
        Self {
            terminals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate ACP-compliant terminal ID with "term_" prefix
    fn generate_terminal_id(&self) -> String {
        format!("term_{}", ulid::Ulid::new())
    }

    /// Create a new terminal session
    pub async fn create_terminal(&self, working_dir: Option<String>) -> crate::Result<String> {
        let terminal_id = self.generate_terminal_id();
        let working_dir = working_dir
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            });

        let session = TerminalSession {
            process: None,
            working_dir,
            environment: std::env::vars().collect(),
            command: None,
            args: Vec::new(),
            session_id: None,
            output_byte_limit: 1_048_576, // 1MB default
            output_buffer: Vec::new(),
            buffer_truncated: false,
        };

        let mut terminals = self.terminals.write().await;
        terminals.insert(terminal_id.clone(), session);

        tracing::info!("Created terminal session: {}", terminal_id);
        Ok(terminal_id)
    }

    /// Create ACP-compliant terminal session with command and all parameters
    ///
    /// This method creates a new terminal session following the Anthropic Computer Protocol
    /// specification. It validates the session ID, resolves the working directory,
    /// prepares environment variables, and creates the terminal with proper output buffering.
    ///
    /// # Arguments
    /// * `session_manager` - Manager for session validation and retrieval
    /// * `params` - Terminal creation parameters including command, args, env, etc.
    ///
    /// # Returns
    /// * `Ok(String)` - The unique terminal ID (ULID format) on success
    /// * `Err(AgentError)` - Protocol error for invalid parameters or session issues
    pub async fn create_terminal_with_command(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalCreateParams,
    ) -> crate::Result<String> {
        // 1. Validate session ID
        self.validate_session_id(session_manager, &params.session_id)
            .await?;

        // 2. Generate ACP-compliant terminal ID
        let terminal_id = self.generate_terminal_id();

        // 3. Resolve working directory (use session cwd if not specified)
        let working_dir = self
            .resolve_working_directory(session_manager, &params.session_id, params.cwd.as_deref())
            .await?;

        // 4. Prepare environment variables
        let environment = self.prepare_environment(params.env.unwrap_or_default())?;

        // 5. Create enhanced terminal session
        let session = TerminalSession {
            process: None,
            working_dir,
            environment,
            command: Some(params.command),
            args: params.args.unwrap_or_default(),
            session_id: Some(params.session_id),
            output_byte_limit: params.output_byte_limit.unwrap_or(1_048_576), // 1MB default
            output_buffer: Vec::new(),
            buffer_truncated: false,
        };

        // 6. Register terminal
        let mut terminals = self.terminals.write().await;
        terminals.insert(terminal_id.clone(), session);

        tracing::info!("Created ACP terminal session: {}", terminal_id);
        Ok(terminal_id)
    }

    /// Validate session ID exists and is properly formatted
    async fn validate_session_id(
        &self,
        session_manager: &crate::session::SessionManager,
        session_id: &str,
    ) -> crate::Result<()> {
        let session_ulid = session_id.parse::<ulid::Ulid>().map_err(|_| {
            crate::AgentError::Protocol(format!("Invalid session ID format: {}", session_id))
        })?;

        session_manager.get_session(&session_ulid)?.ok_or_else(|| {
            crate::AgentError::Protocol(format!("Session not found: {}", session_id))
        })?;

        Ok(())
    }

    /// Resolve working directory from session or parameter
    pub async fn resolve_working_directory(
        &self,
        session_manager: &crate::session::SessionManager,
        session_id: &str,
        cwd_param: Option<&str>,
    ) -> crate::Result<std::path::PathBuf> {
        if let Some(cwd) = cwd_param {
            // Use provided working directory, validate it's absolute
            let path = std::path::PathBuf::from(cwd);
            if !path.is_absolute() {
                return Err(crate::AgentError::Protocol(format!(
                    "Working directory must be absolute path: {}",
                    cwd
                )));
            }
            Ok(path)
        } else {
            // Use session's working directory
            let session_ulid = session_id.parse::<ulid::Ulid>().map_err(|_| {
                crate::AgentError::Protocol(format!("Invalid session ID format: {}", session_id))
            })?;

            let session = session_manager.get_session(&session_ulid)?.ok_or_else(|| {
                crate::AgentError::Protocol(format!("Session not found: {}", session_id))
            })?;

            Ok(session.cwd)
        }
    }

    /// Prepare environment variables by merging custom with system environment
    pub fn prepare_environment(
        &self,
        env_vars: Vec<EnvVariable>,
    ) -> crate::Result<HashMap<String, String>> {
        let mut environment: HashMap<String, String> = std::env::vars().collect();

        // Apply custom environment variables, overriding system ones
        for env_var in env_vars {
            if env_var.name.is_empty() {
                return Err(crate::AgentError::Protocol(
                    "Environment variable name cannot be empty".to_string(),
                ));
            }
            environment.insert(env_var.name, env_var.value);
        }

        Ok(environment)
    }

    /// Execute a command in the specified terminal session
    pub async fn execute_command(&self, terminal_id: &str, command: &str) -> crate::Result<String> {
        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id).ok_or_else(|| {
            crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id))
        })?;

        tracing::info!("Executing command in terminal {}: {}", terminal_id, command);

        // Parse command and arguments
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(crate::AgentError::ToolExecution(
                "Empty command".to_string(),
            ));
        }

        let program = parts[0];
        let args = &parts[1..];

        // Execute command
        let output = Command::new(program)
            .args(args)
            .current_dir(&session.working_dir)
            .envs(&session.environment)
            .output()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to execute command: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let result = if output.status.success() {
            if stdout.is_empty() {
                "Command completed successfully (exit code: 0)".to_string()
            } else {
                format!("Command output:\n{}", stdout)
            }
        } else {
            let exit_code = output.status.code().unwrap_or(-1);
            if stderr.is_empty() {
                format!("Command failed (exit code: {})", exit_code)
            } else {
                format!("Command failed (exit code: {}):\n{}", exit_code, stderr)
            }
        };

        tracing::info!(
            "Command completed with exit code: {:?}",
            output.status.code()
        );
        Ok(result)
    }

    /// Change the working directory for a terminal session
    pub async fn change_directory(&self, terminal_id: &str, path: &str) -> crate::Result<String> {
        let mut terminals = self.terminals.write().await;
        let session = terminals.get_mut(terminal_id).ok_or_else(|| {
            crate::AgentError::ToolExecution(format!("Terminal {} not found", terminal_id))
        })?;

        let new_path = if std::path::Path::new(path).is_absolute() {
            std::path::PathBuf::from(path)
        } else {
            session.working_dir.join(path)
        };

        if new_path.exists() && new_path.is_dir() {
            session.working_dir = new_path.canonicalize().map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to resolve path: {}", e))
            })?;

            tracing::info!("Changed directory to: {}", session.working_dir.display());
            Ok(format!(
                "Changed directory to: {}",
                session.working_dir.display()
            ))
        } else {
            Err(crate::AgentError::ToolExecution(format!(
                "Directory does not exist: {}",
                path
            )))
        }
    }

    /// Remove a terminal session
    pub async fn remove_terminal(&self, terminal_id: &str) -> crate::Result<()> {
        let mut terminals = self.terminals.write().await;
        if let Some(mut session) = terminals.remove(terminal_id) {
            if let Some(mut process) = session.process.take() {
                let _ = process.kill().await;
            }
            tracing::info!("Removed terminal session: {}", terminal_id);
        }
        Ok(())
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalSession {
    /// Add output data to the buffer, enforcing byte limits
    pub fn add_output(&mut self, data: &[u8]) {
        // Always append the new data first
        self.output_buffer.extend_from_slice(data);

        // Then truncate from beginning if we exceed the limit
        let limit = self.output_byte_limit as usize;
        if self.output_buffer.len() > limit {
            let excess = self.output_buffer.len() - limit;
            self.output_buffer.drain(0..excess);
            self.buffer_truncated = true;
        }
    }

    /// Get output as UTF-8 string
    pub fn get_output_string(&self) -> String {
        String::from_utf8_lossy(&self.output_buffer).to_string()
    }

    /// Check if output buffer has been truncated
    pub fn is_output_truncated(&self) -> bool {
        self.buffer_truncated
    }

    /// Get current buffer size in bytes
    pub fn get_buffer_size(&self) -> usize {
        self.output_buffer.len()
    }

    /// Clear the output buffer
    pub fn clear_output(&mut self) {
        self.output_buffer.clear();
        self.buffer_truncated = false;
    }
}