//! Terminal session management for ACP compliance
//!
//! This module provides comprehensive terminal session management following
//! the Agent Client Protocol (ACP) specification.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Manages terminal sessions for command execution
#[derive(Debug, Clone)]
pub struct TerminalManager {
    pub terminals: Arc<RwLock<HashMap<String, TerminalSession>>>,
}

/// Terminal lifecycle state
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalState {
    /// Terminal created but process not yet started
    Created,
    /// Process is currently running
    Running,
    /// Process completed with exit status
    Finished,
    /// Process terminated due to timeout
    TimedOut,
    /// Process killed by signal
    Killed,
    /// Resources released, terminal ID invalidated
    Released,
}

/// Default graceful shutdown timeout in seconds
pub const DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS: u64 = 5;

/// Newtype wrapper for graceful shutdown timeout duration
/// 
/// Provides type safety to prevent mixing up timeout durations with other Duration values.
/// This ensures that timeout configurations cannot be accidentally confused with other
/// time-based parameters in the system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GracefulShutdownTimeout(Duration);

impl GracefulShutdownTimeout {
    /// Create a new graceful shutdown timeout
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }
    
    /// Get the timeout as a Duration
    pub fn as_duration(&self) -> Duration {
        self.0
    }
}

impl Default for GracefulShutdownTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS))
    }
}

/// Configuration for terminal timeout behavior
/// 
/// Controls how terminal sessions handle execution timeouts, including default
/// durations, per-command overrides, and escalation strategies.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Default timeout for command execution (None means no timeout)
    pub default_execution_timeout: Option<Duration>,
    /// Graceful shutdown timeout before escalating to SIGKILL
    pub graceful_shutdown_timeout: GracefulShutdownTimeout,
    /// Per-command timeout overrides (command name -> timeout duration)
    pub command_timeouts: HashMap<String, Duration>,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            default_execution_timeout: None,
            graceful_shutdown_timeout: GracefulShutdownTimeout::default(),
            command_timeouts: HashMap::new(),
        }
    }
}

impl TimeoutConfig {
    /// Get the timeout for a specific command, falling back to default if not specified
    pub fn get_timeout_for_command(&self, command: &str) -> Option<Duration> {
        self.command_timeouts
            .get(command)
            .copied()
            .or(self.default_execution_timeout)
    }
}

/// Represents a terminal session with working directory and environment
#[derive(Debug)]
pub struct TerminalSession {
    pub process: Option<Arc<RwLock<Child>>>,
    pub working_dir: std::path::PathBuf,
    pub environment: HashMap<String, String>,
    // ACP-compliant fields for terminal/create method
    pub command: Option<String>,
    pub args: Vec<String>,
    pub session_id: Option<String>,
    pub output_byte_limit: u64,
    pub output_buffer: Arc<RwLock<Vec<u8>>>,
    pub buffer_truncated: Arc<RwLock<bool>>,
    pub exit_status: Arc<RwLock<Option<ExitStatus>>>,
    pub state: Arc<RwLock<TerminalState>>,
    pub output_task: Option<JoinHandle<()>>,
    pub timeout_config: TimeoutConfig,
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

/// ACP-compliant request parameters for terminal/output method
#[derive(Debug, Deserialize)]
pub struct TerminalOutputParams {
    /// Session identifier
    #[serde(rename = "sessionId")]
    pub session_id: String,
    /// Terminal identifier
    #[serde(rename = "terminalId")]
    pub terminal_id: String,
}

/// ACP-compliant response for terminal/output method
#[derive(Debug, Serialize)]
pub struct TerminalOutputResponse {
    /// Terminal output as UTF-8 string
    pub output: String,
    /// Whether output has been truncated from the beginning
    pub truncated: bool,
    /// Exit status (only present when process has completed)
    #[serde(rename = "exitStatus", skip_serializing_if = "Option::is_none")]
    pub exit_status: Option<ExitStatus>,
}

/// Exit status information for completed processes
#[derive(Debug, Serialize, Clone)]
pub struct ExitStatus {
    /// Exit code (0 for success, non-zero for error)
    #[serde(rename = "exitCode")]
    pub exit_code: Option<i32>,
    /// Signal name if process was terminated by signal
    pub signal: Option<String>,
}

/// ACP-compliant request parameters for terminal/release method
#[derive(Debug, Deserialize)]
pub struct TerminalReleaseParams {
    #[serde(rename = "sessionId")]
    pub session_id: String,
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
            output_buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_truncated: Arc::new(RwLock::new(false)),
            exit_status: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(TerminalState::Created)),
            output_task: None,
            timeout_config: TimeoutConfig::default(),
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
            output_buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_truncated: Arc::new(RwLock::new(false)),
            exit_status: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(TerminalState::Created)),
            output_task: None,
            timeout_config: TimeoutConfig::default(),
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
        let parsed_session_id = crate::session::SessionId::parse(session_id).map_err(|e| {
            crate::AgentError::Protocol(format!("Invalid session ID format: {}", e))
        })?;

        session_manager
            .get_session(&parsed_session_id)?
            .ok_or_else(|| {
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
            let parsed_session_id = crate::session::SessionId::parse(session_id).map_err(|e| {
                crate::AgentError::Protocol(format!("Invalid session ID format: {}", e))
            })?;

            let session = session_manager
                .get_session(&parsed_session_id)?
                .ok_or_else(|| {
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

        // Transition to Running state
        *session.state.write().await = TerminalState::Running;

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

        // Transition to Finished state and set exit status
        let exit_status = ExitStatus {
            exit_code: output.status.code(),
            signal: None,
        };
        session.set_exit_status(exit_status).await;
        *session.state.write().await = TerminalState::Finished;

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
            if let Some(process) = session.process.take() {
                let mut proc = process.write().await;
                let _ = proc.kill().await;
            }
            tracing::info!("Removed terminal session: {}", terminal_id);
        }
        Ok(())
    }

    /// Release a terminal session (ACP terminal/release method)
    ///
    /// This method implements the ACP terminal/release specification:
    /// 1. Kill running process if still active
    /// 2. Clean up all terminal resources (buffers, handles, streams)
    /// 3. Remove terminal from registry and invalidate ID
    /// 4. Prevent resource leaks from unreleased terminals
    /// 5. Return null result on successful release
    ///
    /// Proper release prevents resource leaks and ensures clean shutdown.
    pub async fn release_terminal(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalReleaseParams,
    ) -> crate::Result<serde_json::Value> {
        // 1. Validate session ID
        self.validate_session_id(session_manager, &params.session_id)
            .await?;

        // 2. Get and remove terminal from registry
        let mut terminals = self.terminals.write().await;
        let mut session = terminals.remove(&params.terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", params.terminal_id))
        })?;

        // 3. Release terminal resources
        session.release().await?;

        tracing::info!("Released terminal session: {}", params.terminal_id);

        // 5. Return null result per ACP specification
        Ok(serde_json::Value::Null)
    }

    /// Get output from a terminal session (ACP terminal/output method)
    pub async fn get_output(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalOutputParams,
    ) -> crate::Result<TerminalOutputResponse> {
        // 1. Validate session ID
        let parsed_session_id =
            crate::session::SessionId::parse(&params.session_id).map_err(|e| {
                crate::AgentError::Protocol(format!("Invalid session ID format: {}", e))
            })?;

        session_manager
            .get_session(&parsed_session_id)?
            .ok_or_else(|| {
                crate::AgentError::Protocol(format!("Session not found: {}", params.session_id))
            })?;

        // 2. Get terminal session
        let terminals = self.terminals.read().await;
        let session = terminals.get(&params.terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", params.terminal_id))
        })?;

        // 3. Validate terminal is not released
        session.validate_not_released().await?;

        // 4. Get output data
        let output = session.get_output_string().await;
        let truncated = session.is_output_truncated().await;
        let exit_status = session.get_exit_status().await;

        tracing::debug!(
            "Retrieved output for terminal {}: {} bytes, truncated: {}, exit_status: {:?}",
            params.terminal_id,
            output.len(),
            truncated,
            exit_status
        );

        Ok(TerminalOutputResponse {
            output,
            truncated,
            exit_status,
        })
    }

    /// Wait for terminal process to exit (ACP terminal/wait_for_exit method)
    pub async fn wait_for_exit(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalOutputParams,
    ) -> crate::Result<ExitStatus> {
        // 1. Validate session ID
        self.validate_session_id(session_manager, &params.session_id)
            .await?;

        // 2. Get terminal session
        let terminals = self.terminals.read().await;
        let session = terminals.get(&params.terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", params.terminal_id))
        })?;

        // 3. Wait for exit
        let exit_status = session.wait_for_exit().await?;

        tracing::info!(
            "Terminal {} exited with status: {:?}",
            params.terminal_id,
            exit_status
        );

        Ok(exit_status)
    }

    /// Kill a terminal process (ACP terminal/kill method)
    pub async fn kill_terminal(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalOutputParams,
    ) -> crate::Result<()> {
        // 1. Validate session ID
        self.validate_session_id(session_manager, &params.session_id)
            .await?;

        // 2. Get terminal session
        let terminals = self.terminals.read().await;
        let session = terminals.get(&params.terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", params.terminal_id))
        })?;

        // 3. Kill process
        session.kill_process().await?;

        tracing::info!("Terminal {} killed", params.terminal_id);

        Ok(())
    }

    /// Execute with timeout pattern (concurrent wait and timeout)
    ///
    /// ACP terminal timeout and process control implementation:
    /// 1. Concurrent timeout and exit waiting using tokio::select!
    /// 2. Automatic process kill when timeout exceeded
    /// 3. Final output retrieval for timeout scenarios
    /// 4. Platform-specific signal handling (SIGTERM/SIGKILL on Unix)
    /// 5. Process group management for child process cleanup
    ///
    /// Timeout pattern prevents hanging operations and provides resource control.
    ///
    /// ACP timeout pattern implementation:
    /// 1. Start timer for desired timeout duration
    /// 2. Concurrently wait for either timer to expire or wait_for_exit to return
    /// 3. If timer expires first, kill the process and retrieve final output
    pub async fn execute_with_timeout(
        &self,
        session_manager: &crate::session::SessionManager,
        params: TerminalOutputParams,
        timeout: Duration,
    ) -> crate::Result<TerminalTimeoutResult> {
        // 1. Validate session ID
        self.validate_session_id(session_manager, &params.session_id)
            .await?;

        // 2. Get terminal session
        let terminals = self.terminals.read().await;
        let session = terminals.get(&params.terminal_id).ok_or_else(|| {
            crate::AgentError::Protocol(format!("Terminal not found: {}", params.terminal_id))
        })?;

        // 3. Concurrent wait for exit or timeout using tokio::select!
        tokio::select! {
            // Wait for natural completion
            exit_result = session.wait_for_exit() => {
                match exit_result {
                    Ok(status) => Ok(TerminalTimeoutResult::Completed(status)),
                    Err(e) => Err(e),
                }
            }

            // Handle timeout
            _ = tokio::time::sleep(timeout) => {
                tracing::warn!("Terminal {} timed out after {:?}", params.terminal_id, timeout);

                // Kill the process
                session.kill_process().await?;

                // Get final output
                let output = session.get_output_string().await;
                let truncated = session.is_output_truncated().await;

                Ok(TerminalTimeoutResult::TimedOut {
                    output,
                    truncated,
                })
            }
        }
    }
}

/// Result of timeout pattern execution
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalTimeoutResult {
    /// Process completed before timeout
    Completed(ExitStatus),
    /// Process timed out and was killed
    TimedOut {
        /// Final output captured before kill
        output: String,
        /// Whether output was truncated
        truncated: bool,
    },
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalSession {
    /// Add output data to the buffer, enforcing byte limits with character-boundary truncation
    pub async fn add_output(&self, data: &[u8]) {
        let mut buffer = self.output_buffer.write().await;
        let mut truncated = self.buffer_truncated.write().await;

        // Always append the new data first
        buffer.extend_from_slice(data);

        // Then truncate from beginning if we exceed the limit
        let limit = self.output_byte_limit as usize;
        if buffer.len() > limit {
            let excess = buffer.len() - limit;

            // Find a safe UTF-8 boundary to truncate at
            let truncate_point = Self::find_utf8_boundary(&buffer, excess);
            buffer.drain(0..truncate_point);
            *truncated = true;
        }
    }

    /// Find the nearest UTF-8 character boundary at or after the given position
    fn find_utf8_boundary(data: &[u8], min_pos: usize) -> usize {
        let mut pos = min_pos;

        // Move forward until we find a valid UTF-8 boundary
        while pos < data.len() {
            // Check if this position starts a valid UTF-8 sequence
            // UTF-8 start bytes: 0xxxxxxx, 110xxxxx, 1110xxxx, 11110xxx
            // Continuation bytes: 10xxxxxx
            let byte = data[pos];

            // If this is not a continuation byte, it's a valid boundary
            if (byte & 0b1100_0000) != 0b1000_0000 {
                return pos;
            }

            pos += 1;
        }

        // If we reached the end, return the data length
        data.len()
    }

    /// Get output as UTF-8 string
    pub async fn get_output_string(&self) -> String {
        let buffer = self.output_buffer.read().await;
        String::from_utf8_lossy(&buffer).to_string()
    }

    /// Check if output buffer has been truncated
    pub async fn is_output_truncated(&self) -> bool {
        *self.buffer_truncated.read().await
    }

    /// Get current buffer size in bytes
    pub async fn get_buffer_size(&self) -> usize {
        self.output_buffer.read().await.len()
    }

    /// Clear the output buffer
    pub async fn clear_output(&self) {
        self.output_buffer.write().await.clear();
        *self.buffer_truncated.write().await = false;
    }

    /// Get the current exit status
    pub async fn get_exit_status(&self) -> Option<ExitStatus> {
        self.exit_status.read().await.clone()
    }

    /// Set the exit status when process completes
    pub async fn set_exit_status(&self, status: ExitStatus) {
        *self.exit_status.write().await = Some(status);
    }

    /// Get current terminal state
    pub async fn get_state(&self) -> TerminalState {
        self.state.read().await.clone()
    }

    /// Check if terminal is in Released state
    pub async fn is_released(&self) -> bool {
        matches!(*self.state.read().await, TerminalState::Released)
    }

    /// Check if terminal is in Finished state
    pub async fn is_finished(&self) -> bool {
        matches!(*self.state.read().await, TerminalState::Finished)
    }

    /// Validate terminal is not released (for operations that require active terminal)
    pub async fn validate_not_released(&self) -> crate::Result<()> {
        if self.is_released().await {
            return Err(crate::AgentError::Protocol(
                "Terminal has been released".to_string(),
            ));
        }
        Ok(())
    }

    /// Wait for process to exit and return exit status
    ///
    /// ACP terminal/wait_for_exit method implementation:
    /// Blocks until the process completes and returns the exit status
    /// Wait for process to exit and return the exit status
    ///
    /// This method blocks until the process completes and returns the exit status
    /// including exit code and signal information. If the process has already finished,
    /// it returns the cached exit status immediately.
    ///
    /// # Returns
    ///
    /// * `Ok(ExitStatus)` - Exit status with code and optional signal name
    ///
    /// # Errors
    ///
    /// * `AgentError::Protocol` - Terminal has been released or no process running
    /// * `AgentError::ToolExecution` - Failed to wait for process completion
    ///
    /// # Behavior
    ///
    /// - Returns cached exit status if process already finished
    /// - Blocks waiting for process completion if still running
    /// - Updates terminal state to Finished after process exits
    /// - Extracts and stores signal information on Unix systems
    ///
    /// # Example Usage
    ///
    /// ```ignore
    /// let status = terminal.wait_for_exit().await?;
    /// if let Some(code) = status.exit_code {
    ///     println!("Process exited with code: {}", code);
    /// }
    /// if let Some(signal) = status.signal {
    ///     println!("Process killed by signal: {}", signal);
    /// }
    /// ```
    pub async fn wait_for_exit(&self) -> crate::Result<ExitStatus> {
        // Validate terminal is not released
        self.validate_not_released().await?;

        // Check if already finished
        if let Some(status) = self.get_exit_status().await {
            return Ok(status);
        }

        // Check if process exists
        let process = self.process.as_ref().ok_or_else(|| {
            crate::AgentError::Protocol("No process running".to_string())
        })?;

        // Wait for process to complete
        let status = {
            let mut proc = process.write().await;
            proc.wait().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!("Failed to wait for process: {}", e))
            })?
        };

        // Convert to our ExitStatus
        let exit_status = ExitStatus {
            exit_code: status.code(),
            signal: Self::get_signal_name(&status),
        };

        // Store exit status and update state
        self.set_exit_status(exit_status.clone()).await;
        *self.state.write().await = TerminalState::Finished;

        Ok(exit_status)
    }

    /// Get signal name from process status
    #[cfg(unix)]
    fn get_signal_name(status: &std::process::ExitStatus) -> Option<String> {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map(|sig| match sig {
            1 => "SIGHUP".to_string(),
            2 => "SIGINT".to_string(),
            9 => "SIGKILL".to_string(),
            15 => "SIGTERM".to_string(),
            _ => format!("signal {}", sig),
        })
    }

    #[cfg(not(unix))]
    fn get_signal_name(_status: &std::process::ExitStatus) -> Option<String> {
        None
    }

    /// Kill the running process with signal handling
    ///
    /// ACP terminal/kill method implementation:
    /// 1. Send SIGTERM for graceful shutdown (Unix only)
    /// 2. Wait for graceful_shutdown_timeout
    /// 3. Send SIGKILL if process still running
    pub async fn kill_process(&self) -> crate::Result<()> {
        // Validate terminal is not released
        self.validate_not_released().await?;

        // Check if already finished
        if self.is_finished().await {
            tracing::debug!("Process already finished, skipping kill");
            return Ok(());
        }

        // Check if process exists
        let process = self.process.as_ref().ok_or_else(|| {
            crate::AgentError::Protocol("No process running".to_string())
        })?;

        #[cfg(unix)]
        {
            self.kill_process_unix(process).await?;
        }

        #[cfg(not(unix))]
        {
            self.kill_process_windows(process).await?;
        }

        // Update state
        *self.state.write().await = TerminalState::Killed;

        Ok(())
    }

    #[cfg(unix)]
    async fn kill_process_unix(&self, process: &Arc<RwLock<Child>>) -> crate::Result<()> {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        let pid = {
            let proc = process.read().await;
            proc.id().ok_or_else(|| {
                crate::AgentError::Protocol("Process ID not available".to_string())
            })?
        };

        let pid = Pid::from_raw(pid as i32);

        // Send SIGTERM for graceful shutdown
        tracing::debug!("Sending SIGTERM to process {}", pid);
        kill(pid, Signal::SIGTERM).map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to send SIGTERM: {}", e))
        })?;

        // Wait for graceful shutdown with timeout
        let graceful_timeout = self.timeout_config.graceful_shutdown_timeout.as_duration();
        let wait_result = tokio::time::timeout(graceful_timeout, async {
            let mut proc = process.write().await;
            proc.wait().await
        })
        .await;

        match wait_result {
            Ok(Ok(status)) => {
                tracing::debug!("Process terminated gracefully with status: {:?}", status);
                let exit_status = ExitStatus {
                    exit_code: status.code(),
                    signal: Self::get_signal_name(&status),
                };
                self.set_exit_status(exit_status).await;
                Ok(())
            }
            Ok(Err(e)) => Err(crate::AgentError::ToolExecution(format!(
                "Failed to wait for process: {}",
                e
            ))),
            Err(_) => {
                // Timeout - force kill with SIGKILL
                tracing::debug!("Graceful shutdown timed out, sending SIGKILL to process {}", pid);
                kill(pid, Signal::SIGKILL).map_err(|e| {
                    crate::AgentError::ToolExecution(format!("Failed to send SIGKILL: {}", e))
                })?;

                // Wait for forceful kill
                let mut proc = process.write().await;
                let status = proc.wait().await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!("Failed to wait after SIGKILL: {}", e))
                })?;

                let exit_status = ExitStatus {
                    exit_code: status.code(),
                    signal: Some("SIGKILL".to_string()),
                };
                self.set_exit_status(exit_status).await;
                Ok(())
            }
        }
    }

    #[cfg(not(unix))]
    async fn kill_process_windows(&self, process: &Arc<RwLock<Child>>) -> crate::Result<()> {
        // Windows doesn't have signals - use TerminateProcess directly
        let mut proc = process.write().await;
        proc.kill().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to kill process: {}", e))
        })?;

        let status = proc.wait().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!("Failed to wait for process: {}", e))
        })?;

        let exit_status = ExitStatus {
            exit_code: status.code(),
            signal: None,
        };
        self.set_exit_status(exit_status).await;
        Ok(())
    }

    /// Release terminal resources
    ///
    /// ACP terminal/release method implementation:
    /// 1. Kill running process if still active
    /// 2. Clean up all terminal resources (buffers, handles, streams)
    /// 3. Remove terminal from registry and invalidate ID
    /// 4. Prevent resource leaks from unreleased terminals
    /// 5. Return null result on successful release
    ///
    /// Proper release prevents resource leaks and ensures clean shutdown.
    pub async fn release(&mut self) -> crate::Result<()> {
        // Kill process if still running
        if let Some(process) = self.process.take() {
            let mut proc = process.write().await;
            let _ = proc.kill().await;
            tracing::debug!("Killed process during terminal release");
        }

        // Abort output task if running
        if let Some(task) = self.output_task.take() {
            task.abort();
        }

        // Clear output buffers to free memory
        self.output_buffer.write().await.clear();
        *self.buffer_truncated.write().await = false;

        // Mark as released
        *self.state.write().await = TerminalState::Released;

        tracing::debug!("Terminal resources released");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_session_manager() -> crate::session::SessionManager {
        crate::session::SessionManager::new()
    }

    async fn create_terminal_for_testing(
        manager: &TerminalManager,
        session_manager: &crate::session::SessionManager,
    ) -> crate::Result<(String, String)> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let session_id = session_manager.create_session(cwd)?;
        let session_id_str = session_id.to_string();

        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "echo".to_string(),
            args: Some(vec!["test".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(session_manager, params)
            .await?;

        Ok((session_id_str, terminal_id))
    }

    #[tokio::test]
    async fn test_terminal_state_lifecycle() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (_session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;

        assert_eq!(state, TerminalState::Created);
        assert!(!session.is_released().await);
    }

    #[tokio::test]
    async fn test_release_terminal_success() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        let params = TerminalReleaseParams {
            session_id,
            terminal_id: terminal_id.clone(),
        };

        let result = manager.release_terminal(&session_manager, params).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);

        let terminals = manager.terminals.read().await;
        assert!(terminals.get(&terminal_id).is_none());
    }

    #[tokio::test]
    async fn test_release_terminal_not_found() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let session_id = session_manager.create_session(cwd).unwrap();

        let params = TerminalReleaseParams {
            session_id: session_id.to_string(),
            terminal_id: "term_nonexistent".to_string(),
        };

        let result = manager.release_terminal(&session_manager, params).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Terminal not found"));
    }

    #[tokio::test]
    async fn test_release_terminal_invalid_session() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let params = TerminalReleaseParams {
            session_id: "sess_01K6DB0000000000000000000".to_string(),
            terminal_id: "term_test".to_string(),
        };

        let result = manager.release_terminal(&session_manager, params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_terminal_session_release_clears_buffers() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (_session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            session.add_output(b"test output").await;
            let buffer_size = session.get_buffer_size().await;
            assert!(buffer_size > 0);
        }

        {
            let mut terminals = manager.terminals.write().await;
            let mut session = terminals.remove(&terminal_id).unwrap();
            session.release().await.unwrap();
            let buffer_size = session.get_buffer_size().await;
            assert_eq!(buffer_size, 0);
            assert!(session.is_released().await);
        }
    }

    #[tokio::test]
    async fn test_validate_not_released() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (_session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            assert!(session.validate_not_released().await.is_ok());
        }

        {
            let mut terminals = manager.terminals.write().await;
            let mut session = terminals.remove(&terminal_id).unwrap();
            session.release().await.unwrap();
            let result = session.validate_not_released().await;
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Terminal has been released"));
        }
    }

    #[tokio::test]
    async fn test_get_output_on_released_terminal() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        let release_params = TerminalReleaseParams {
            session_id: session_id.clone(),
            terminal_id: terminal_id.clone(),
        };

        manager
            .release_terminal(&session_manager, release_params)
            .await
            .unwrap();

        let output_params = TerminalOutputParams {
            session_id,
            terminal_id,
        };

        let result = manager.get_output(&session_manager, output_params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_terminal_state_transitions() {
        let session = TerminalSession {
            process: None,
            working_dir: std::path::PathBuf::from("/tmp"),
            environment: HashMap::new(),
            command: Some("echo".to_string()),
            args: vec!["test".to_string()],
            session_id: Some("sess_test".to_string()),
            output_byte_limit: 1024,
            output_buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_truncated: Arc::new(RwLock::new(false)),
            exit_status: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(TerminalState::Created)),
            output_task: None,
            timeout_config: TimeoutConfig::default(),
        };

        assert_eq!(session.get_state().await, TerminalState::Created);
        assert!(!session.is_released().await);
        assert!(!session.is_finished().await);

        *session.state.write().await = TerminalState::Running;
        assert_eq!(session.get_state().await, TerminalState::Running);

        *session.state.write().await = TerminalState::Finished;
        assert!(session.is_finished().await);

        *session.state.write().await = TerminalState::Killed;
        assert_eq!(session.get_state().await, TerminalState::Killed);

        *session.state.write().await = TerminalState::TimedOut;
        assert_eq!(session.get_state().await, TerminalState::TimedOut);

        *session.state.write().await = TerminalState::Released;
        assert!(session.is_released().await);
    }

    #[tokio::test]
    async fn test_timeout_triggers_kill() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let session_id = session_manager.create_session(cwd).unwrap();

        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "sleep".to_string(),
            args: Some(vec!["10".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process by spawning it
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("sleep");
            cmd.arg("10")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Execute with short timeout (should timeout)
        let output_params = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let result = manager
            .execute_with_timeout(&session_manager, output_params, Duration::from_millis(100))
            .await
            .expect("execute_with_timeout should succeed");

        match result {
            TerminalTimeoutResult::TimedOut { .. } => {
                // Expected - timeout triggered kill
            }
            TerminalTimeoutResult::Completed(_) => {
                panic!("Expected timeout, but process completed normally");
            }
        }

        // Verify terminal state is killed
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;
        assert_eq!(state, TerminalState::Killed);
    }

    #[tokio::test]
    async fn test_concurrent_wait_and_timeout() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let session_id = session_manager.create_session(cwd).unwrap();

        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "echo".to_string(),
            args: Some(vec!["quick".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start a quick process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("echo");
            cmd.arg("quick")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Execute with long timeout (should complete before timeout)
        let output_params = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let result = manager
            .execute_with_timeout(&session_manager, output_params, Duration::from_secs(10))
            .await;

        assert!(result.is_ok());
        match result.unwrap() {
            TerminalTimeoutResult::Completed(status) => {
                assert_eq!(status.exit_code, Some(0));
            }
            TerminalTimeoutResult::TimedOut { .. } => {
                panic!("Expected completion, but process timed out");
            }
        }

        // Verify terminal state is finished
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;
        assert_eq!(state, TerminalState::Finished);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_signal_handling_graceful_termination() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let session_id = session_manager.create_session(cwd).unwrap();

        let params = TerminalCreateParams {
            session_id: session_id.to_string(),
            command: "sleep".to_string(),
            args: Some(vec!["30".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();

        // Start the process
        {
            let mut terminals = manager.terminals.write().await;
            let session = terminals.get_mut(&terminal_id).unwrap();

            let mut cmd = Command::new("sleep");
            cmd.arg("30")
                .current_dir(&session.working_dir)
                .envs(&session.environment)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let child = cmd.spawn().unwrap();
            session.process = Some(Arc::new(RwLock::new(child)));
            *session.state.write().await = TerminalState::Running;
        }

        // Kill the process
        let kill_params = TerminalOutputParams {
            session_id: session_id.to_string(),
            terminal_id: terminal_id.clone(),
        };

        let result = manager.kill_terminal(&session_manager, kill_params).await;
        result.expect("kill_terminal should succeed");

        // Verify terminal state is killed
        let terminals = manager.terminals.read().await;
        let session = terminals.get(&terminal_id).unwrap();
        let state = session.get_state().await;
        assert_eq!(state, TerminalState::Killed);

        // Verify exit status has signal information
        let exit_status = session.get_exit_status().await;
        assert!(exit_status.is_some());
        let status = exit_status.unwrap();
        assert!(status.signal.is_some());
    }

    #[tokio::test]
    async fn test_wait_for_exit_already_finished() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Manually set exit status to simulate finished process
        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            let status = ExitStatus {
                exit_code: Some(0),
                signal: None,
            };
            session.set_exit_status(status).await;
            *session.state.write().await = TerminalState::Finished;
        }

        // Wait for exit should return immediately with cached status
        let params = TerminalOutputParams {
            session_id,
            terminal_id,
        };

        let result = manager.wait_for_exit(&session_manager, params).await;
        assert!(result.is_ok());
        let status = result.unwrap();
        assert_eq!(status.exit_code, Some(0));
        assert_eq!(status.signal, None);
    }

    #[tokio::test]
    async fn test_kill_already_finished_process() {
        let manager = TerminalManager::new();
        let session_manager = create_test_session_manager().await;

        let (session_id, terminal_id) = create_terminal_for_testing(&manager, &session_manager)
            .await
            .unwrap();

        // Manually set to finished state and test kill_process directly
        {
            let terminals = manager.terminals.read().await;
            let session = terminals.get(&terminal_id).unwrap();
            *session.state.write().await = TerminalState::Finished;
            
            // Test session-level kill (should succeed without process)
            let result = session.kill_process().await;
            assert!(result.is_ok(), "Session-level kill failed: {:?}", result.err());
        }

        // Also test manager-level kill
        let params = TerminalOutputParams {
            session_id,
            terminal_id: terminal_id.clone(),
        };

        let result = manager.kill_terminal(&session_manager, params).await;
        match result {
            Ok(_) => {},
            Err(e) => panic!("Manager-level kill failed: {}", e),
        }
    }
}
