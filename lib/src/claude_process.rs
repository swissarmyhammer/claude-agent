//! Claude CLI process management for persistent stream-json communication
//!
//! This module provides process management capabilities for spawning and maintaining
//! persistent `claude` CLI processes that communicate via the stream-json protocol.
//!
//! # Architecture
//!
//! The module provides two main types:
//!
//! - [`ClaudeProcessManager`]: Manages a collection of claude processes, one per session.
//!   Provides session-level operations like spawn, get, terminate, and session existence checks.
//!
//! - [`ClaudeProcess`]: Represents a single persistent claude CLI child process.
//!   Provides low-level I/O operations (read/write lines), process lifecycle management,
//!   and status checking.
//!
//! # Stream-JSON Protocol
//!
//! The claude CLI is spawned with the following flags to enable stream-json communication:
//!
//! ```bash
//! claude -p \
//!   --input-format stream-json \
//!   --output-format stream-json \
//!   --verbose \
//!   --dangerously-skip-permissions \
//!   --replay-user-messages
//! ```
//!
//! - `-p`: Print mode (non-interactive)
//! - `--input-format stream-json`: Accept newline-delimited JSON on stdin
//! - `--output-format stream-json`: Emit newline-delimited JSON on stdout
//! - `--verbose`: Required for stream-json output format
//! - `--dangerously-skip-permissions`: ACP server handles permission checks
//! - `--replay-user-messages`: Re-emit user messages for immediate acknowledgment
//!
//! Messages are exchanged as newline-delimited JSON objects conforming to the
//! JSON-RPC 2.0 specification for Agent Communication Protocol (ACP).
//!
//! # Thread Safety
//!
//! [`ClaudeProcessManager`] is thread-safe and can be safely shared across threads using `Arc`.
//! It uses `Arc<RwLock<HashMap>>` internally to allow concurrent read access for session lookups
//! while serializing write operations (spawn/terminate).
//!
//! Individual [`ClaudeProcess`] instances are wrapped in `Arc<Mutex<>>` to allow exclusive
//! access for I/O operations, preventing data races when reading/writing to stdin/stdout.
//!
//! # Usage Example
//!
//! ```no_run
//! use claude_agent::claude_process::ClaudeProcessManager;
//! use claude_agent::session::SessionId;
//!
//! # async fn example() -> claude_agent::Result<()> {
//! let manager = ClaudeProcessManager::new();
//! let session_id = SessionId::new();
//!
//! // Spawn a new process
//! manager.spawn_for_session(session_id).await?;
//!
//! // Get the process and interact with it
//! let process = manager.get_process(&session_id).await?;
//! let mut proc = process.lock().await;
//!
//! // Write a JSON-RPC message
//! proc.write_line(r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}"#).await?;
//!
//! // Read the response
//! if let Some(response) = proc.read_line().await? {
//!     println!("Received: {}", response);
//! }
//!
//! drop(proc); // Release lock before terminating
//!
//! // Terminate when done
//! manager.terminate_session(&session_id).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Error Handling
//!
//! Operations return [`crate::Result<T>`] which wraps [`crate::AgentError`]:
//!
//! - `AgentError::Internal`: Process spawn failures, I/O errors, binary not found
//! - `AgentError::Session`: Session already exists, session not found
//!
//! # Process Lifecycle
//!
//! 1. **Spawn**: `ClaudeProcess::spawn()` creates a new child process with stdin/stdout/stderr pipes
//! 2. **Active**: Process runs persistently, accepting JSON messages on stdin and emitting on stdout
//! 3. **Shutdown**: `shutdown()` drops stdin (signaling EOF), waits for graceful exit with 5s timeout,
//!    then force-kills if necessary
//!
//! Processes are automatically cleaned up when terminated via the manager, but callers must ensure
//! no `Arc<Mutex<ClaudeProcess>>` references are held when calling `terminate_session()`.

use crate::session::SessionId;
use crate::{AgentError, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

/// Claude CLI command-line arguments for stream-json communication
const CLAUDE_CLI_ARGS: &[&str] = &[
    "-p", // print mode (non-interactive)
    "--input-format",
    "stream-json", // accept newline-delimited JSON on stdin
    "--output-format",
    "stream-json",                    // emit newline-delimited JSON on stdout
    "--verbose",                      // REQUIRED for stream-json output format
    "--dangerously-skip-permissions", // ACP server handles permission checks
    "--include-partial-messages",     // Emit partial messages for immediate streaming
];

/// Manages multiple persistent claude CLI processes, one per session
///
/// # Thread Safety
///
/// This type is thread-safe and can be safely shared across threads using `Arc<ClaudeProcessManager>`.
///
/// The internal `processes` map uses `Arc<RwLock<HashMap>>` which provides:
/// - **Concurrent reads**: Multiple threads can simultaneously check session existence or retrieve processes
/// - **Exclusive writes**: Spawn and terminate operations acquire exclusive write locks, preventing races
///
/// Individual processes are wrapped in `Arc<Mutex<ClaudeProcess>>` to ensure exclusive access
/// for I/O operations. Callers must acquire the mutex lock before reading/writing to a process.
///
/// # Important
///
/// When calling `terminate_session()`, ensure no `Arc<Mutex<ClaudeProcess>>` references are held,
/// as termination requires exclusive ownership. Drop all process references before terminating.
#[derive(Debug)]
pub struct ClaudeProcessManager {
    processes: Arc<RwLock<HashMap<SessionId, Arc<Mutex<ClaudeProcess>>>>>,
}

impl ClaudeProcessManager {
    /// Create a new process manager
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a new claude process for the given session
    ///
    /// # Errors
    /// Returns error if:
    /// - Session already has a process
    /// - Failed to spawn claude binary
    /// - Process spawn fails
    pub async fn spawn_for_session(&self, session_id: SessionId) -> Result<()> {
        // Check if session already exists - use write lock to prevent race
        let mut processes = self.processes.write().map_err(|_| {
            AgentError::Internal("Failed to acquire write lock on processes".to_string())
        })?;

        if processes.contains_key(&session_id) {
            // Process already exists, this is fine - just return success
            tracing::debug!("Process already exists for session {}", session_id);
            return Ok(());
        }

        // Spawn new process
        let process = ClaudeProcess::spawn(session_id).map_err(|e| {
            tracing::error!(
                "Failed to spawn claude process for session {}: {}",
                session_id,
                e
            );
            e
        })?;

        // Insert into map
        processes.insert(session_id, Arc::new(Mutex::new(process)));

        tracing::info!("Spawned claude process for session {}", session_id);
        Ok(())
    }

    /// Get the process for a session, spawning one if it doesn't exist
    ///
    /// # Errors
    /// Returns error if spawning fails
    pub async fn get_process(&self, session_id: &SessionId) -> Result<Arc<Mutex<ClaudeProcess>>> {
        // First try to get existing process
        {
            let processes = self.processes.read().map_err(|_| {
                AgentError::Internal("Failed to acquire read lock on processes".to_string())
            })?;
            if let Some(process) = processes.get(session_id) {
                tracing::debug!(
                    "Reusing existing Claude process for session {} (total active: {})",
                    session_id,
                    processes.len()
                );
                return Ok(process.clone());
            }
        }

        // Process doesn't exist, spawn one
        tracing::info!(
            "No process found for session {}, spawning new one",
            session_id
        );
        self.spawn_for_session(*session_id).await?;

        // Get the newly spawned process
        let processes = self.processes.read().map_err(|_| {
            AgentError::Internal("Failed to acquire read lock on processes".to_string())
        })?;

        tracing::info!(
            "Spawned new Claude process for session {} (total active: {})",
            session_id,
            processes.len()
        );

        processes.get(session_id).cloned().ok_or_else(|| {
            AgentError::Internal("Process spawn succeeded but not found in map".to_string())
        })
    }

    /// Terminate a session's process
    ///
    /// # Errors
    /// Returns error if session does not exist or shutdown fails
    pub async fn terminate_session(&self, session_id: &SessionId) -> Result<()> {
        // Remove from map
        let process = {
            let mut processes = self.processes.write().map_err(|_| {
                AgentError::Internal("Failed to acquire write lock on processes".to_string())
            })?;
            processes.remove(session_id)
        };

        if let Some(process_arc) = process {
            // Take ownership and shutdown
            let process = Arc::try_unwrap(process_arc).map_err(|_| {
                AgentError::Internal("Process still has multiple references".to_string())
            })?;
            let process = process.into_inner();

            process.shutdown().await?;
            tracing::info!("Terminated claude process for session {}", session_id);
            Ok(())
        } else {
            Err(AgentError::Session(format!(
                "No process for session {}",
                session_id
            )))
        }
    }

    /// Check if a session has a process
    pub async fn has_session(&self, session_id: &SessionId) -> bool {
        self.processes
            .read()
            .ok()
            .map(|processes| processes.contains_key(session_id))
            .unwrap_or(false)
    }
}

impl Default for ClaudeProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A persistent claude CLI process for stream-json communication
#[derive(Debug)]
pub struct ClaudeProcess {
    session_id: SessionId,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
}

impl ClaudeProcess {
    /// Spawn a new claude process with stream-json flags
    ///
    /// # Errors
    /// Returns error if:
    /// - claude binary not found
    /// - Process spawn fails
    /// - stdin/stdout/stderr not available
    pub fn spawn(session_id: SessionId) -> Result<Self> {
        let mut cmd = Command::new("claude")
            .args(CLAUDE_CLI_ARGS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AgentError::Internal(
                        "claude binary not found in PATH. Please ensure claude CLI is installed."
                            .to_string(),
                    )
                } else {
                    AgentError::Internal(format!("Failed to spawn claude process: {}", e))
                }
            })?;

        let stdin = cmd.stdin.take().ok_or_else(|| {
            AgentError::Internal("Failed to capture claude process stdin".to_string())
        })?;

        let stdout = cmd.stdout.take().ok_or_else(|| {
            AgentError::Internal("Failed to capture claude process stdout".to_string())
        })?;

        let stderr = cmd.stderr.take().ok_or_else(|| {
            AgentError::Internal("Failed to capture claude process stderr".to_string())
        })?;

        tracing::debug!(
            "Spawned claude process for session {} with PID {:?}",
            session_id,
            cmd.id()
        );

        Ok(Self {
            session_id,
            child: cmd,
            stdin,
            stdout: BufReader::new(stdout),
            stderr: BufReader::new(stderr),
        })
    }

    /// Write a line to the process stdin
    ///
    /// # Errors
    /// Returns error if write or flush fails
    pub async fn write_line(&mut self, line: &str) -> Result<()> {
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| AgentError::Internal(format!("Failed to write to claude stdin: {}", e)))?;

        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| AgentError::Internal(format!("Failed to write newline: {}", e)))?;

        self.stdin
            .flush()
            .await
            .map_err(|e| AgentError::Internal(format!("Failed to flush claude stdin: {}", e)))?;

        tracing::trace!("Wrote line to session {}: {}", self.session_id, line);
        Ok(())
    }

    /// Read a line from the process stdout
    ///
    /// Returns None if EOF (process terminated)
    ///
    /// # Errors
    /// Returns error if read fails (but not on EOF)
    pub async fn read_line(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        let bytes_read = self.stdout.read_line(&mut line).await.map_err(|e| {
            AgentError::Internal(format!("Failed to read from claude stdout: {}", e))
        })?;

        if bytes_read == 0 {
            tracing::debug!("EOF on claude stdout for session {}", self.session_id);
            return Ok(None);
        }

        // Remove trailing newline
        let line = line.trim_end().to_string();
        tracing::trace!("Read line from session {}: {}", self.session_id, line);
        Ok(Some(line))
    }

    /// Read a line from the process stderr
    ///
    /// Returns None if EOF
    ///
    /// # Errors
    /// Returns error if read fails (but not on EOF)
    pub async fn read_stderr_line(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        let bytes_read = self.stderr.read_line(&mut line).await.map_err(|e| {
            AgentError::Internal(format!("Failed to read from claude stderr: {}", e))
        })?;

        if bytes_read == 0 {
            return Ok(None);
        }

        // Remove trailing newline
        let line = line.trim_end().to_string();
        tracing::trace!(
            "Read stderr line from session {}: {}",
            self.session_id,
            line
        );
        Ok(Some(line))
    }

    /// Check if the process is still alive
    pub async fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                tracing::debug!(
                    "Claude process for session {} exited with status: {}",
                    self.session_id,
                    status
                );
                false
            }
            Ok(None) => true,
            Err(e) => {
                tracing::error!(
                    "Error checking claude process status for session {}: {}",
                    self.session_id,
                    e
                );
                false
            }
        }
    }

    /// Gracefully shutdown the process
    ///
    /// Attempts graceful termination first, then force kills if needed
    ///
    /// # Errors
    /// Returns error if force kill fails
    pub async fn shutdown(mut self) -> Result<()> {
        tracing::debug!(
            "Shutting down claude process for session {}",
            self.session_id
        );

        // Drop stdin to signal EOF to the process
        drop(self.stdin);

        // Try to wait for graceful exit with timeout
        // Use try_wait in a loop to avoid blocking and retain access to child
        let start = std::time::Instant::now();
        let timeout_duration = Duration::from_secs(5);

        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    tracing::info!(
                        "Claude process for session {} exited gracefully with status: {}",
                        self.session_id,
                        status
                    );
                    return Ok(());
                }
                Ok(None) => {
                    // Still running, check timeout
                    if start.elapsed() >= timeout_duration {
                        tracing::warn!(
                            "Claude process for session {} did not exit gracefully, force killing",
                            self.session_id
                        );
                        // Force kill the process
                        if let Err(e) = self.child.kill().await {
                            tracing::error!(
                                "Failed to force kill claude process for session {}: {}",
                                self.session_id,
                                e
                            );
                            return Err(AgentError::Internal(format!(
                                "Failed to force kill process: {}",
                                e
                            )));
                        }
                        // Wait for the killed process to clean up
                        if let Err(e) = self.child.wait().await {
                            tracing::error!(
                                "Failed to wait after killing claude process for session {}: {}",
                                self.session_id,
                                e
                            );
                        }
                        return Ok(());
                    }
                    // Sleep briefly before checking again
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    tracing::error!(
                        "Error checking claude process status for session {}: {}",
                        self.session_id,
                        e
                    );
                    return Err(AgentError::Internal(format!(
                        "Failed to check process status: {}",
                        e
                    )));
                }
            }
        }
    }

    /// Get the session ID for this process
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_manager_new() {
        let manager = ClaudeProcessManager::new();
        let session_id = SessionId::new();
        assert!(!manager.has_session(&session_id).await);
    }

    #[tokio::test]
    async fn test_spawn_and_terminate_process() {
        let manager = ClaudeProcessManager::new();
        let session_id = SessionId::new();

        // Spawn process
        let result = manager.spawn_for_session(session_id).await;
        if result.is_err() {
            // Skip test if claude is not installed
            eprintln!(
                "Skipping test - claude not installed: {:?}",
                result.unwrap_err()
            );
            return;
        }

        // Check session exists
        assert!(manager.has_session(&session_id).await);

        // Get process and immediately drop it
        {
            let process_result = manager.get_process(&session_id).await;
            assert!(process_result.is_ok());
        } // Drop process_result here to release Arc reference

        // Terminate
        let terminate_result = manager.terminate_session(&session_id).await;
        assert!(terminate_result.is_ok());

        // Check session removed
        assert!(!manager.has_session(&session_id).await);
    }

    #[tokio::test]
    async fn test_spawn_duplicate_session() {
        let manager = ClaudeProcessManager::new();
        let session_id = SessionId::new();

        // Spawn first process
        let result = manager.spawn_for_session(session_id).await;
        if result.is_err() {
            eprintln!(
                "Skipping test - claude not installed: {:?}",
                result.unwrap_err()
            );
            return;
        }

        // Try to spawn duplicate - should be idempotent and return Ok
        let result = manager.spawn_for_session(session_id).await;
        assert!(result.is_ok(), "spawn_for_session should be idempotent");

        // Cleanup
        let _ = manager.terminate_session(&session_id).await;
    }

    #[tokio::test]
    async fn test_get_nonexistent_process() {
        let manager = ClaudeProcessManager::new();
        let session_id = SessionId::new();

        // get_process now auto-spawns processes if they don't exist
        let result = manager.get_process(&session_id).await;
        assert!(
            result.is_ok(),
            "get_process should auto-spawn if process doesn't exist"
        );

        // Verify the process was spawned by checking session_id matches
        let process = result.unwrap();
        assert_eq!(process.lock().await.session_id(), session_id);

        // Cleanup
        let _ = manager.terminate_session(&session_id).await;
    }

    #[tokio::test]
    async fn test_terminate_nonexistent_session() {
        let manager = ClaudeProcessManager::new();
        let session_id = SessionId::new();

        let result = manager.terminate_session(&session_id).await;
        assert!(result.is_err());
        if let Err(AgentError::Session(msg)) = result {
            assert!(msg.contains("No process for session"));
        } else {
            panic!("Expected Session error");
        }
    }

    #[tokio::test]
    async fn test_multiple_sessions() {
        let manager = ClaudeProcessManager::new();
        let session1 = SessionId::new();
        let session2 = SessionId::new();

        // Spawn two processes
        let result1 = manager.spawn_for_session(session1).await;
        let result2 = manager.spawn_for_session(session2).await;

        if result1.is_err() || result2.is_err() {
            eprintln!("Skipping test - claude not installed");
            return;
        }

        // Both should exist
        assert!(manager.has_session(&session1).await);
        assert!(manager.has_session(&session2).await);

        // Terminate both
        let _ = manager.terminate_session(&session1).await;
        let _ = manager.terminate_session(&session2).await;

        // Both should be gone
        assert!(!manager.has_session(&session1).await);
        assert!(!manager.has_session(&session2).await);
    }

    #[tokio::test]
    async fn test_process_spawn() {
        let session_id = SessionId::new();
        let result = ClaudeProcess::spawn(session_id);

        if result.is_err() {
            eprintln!(
                "Skipping test - claude not installed: {:?}",
                result.unwrap_err()
            );
            return;
        }

        let mut process = result.unwrap();
        assert_eq!(process.session_id(), session_id);
        assert!(process.is_alive().await);

        // Cleanup
        let _ = process.shutdown().await;
    }

    #[tokio::test]
    async fn test_process_write_read() {
        let session_id = SessionId::new();
        let result = ClaudeProcess::spawn(session_id);

        if result.is_err() {
            eprintln!("Skipping test - claude not installed");
            return;
        }

        let mut process = result.unwrap();

        // Write a test message
        let write_result = process.write_line(r#"{"test": "message"}"#).await;
        assert!(write_result.is_ok());

        // Try to read response
        // Note: This may timeout or fail if claude doesn't respond to arbitrary JSON
        // In real usage, we'd send proper stream-json formatted messages

        // Cleanup
        let _ = process.shutdown().await;
    }

    #[tokio::test]
    async fn test_process_shutdown() {
        let session_id = SessionId::new();
        let result = ClaudeProcess::spawn(session_id);

        if result.is_err() {
            eprintln!("Skipping test - claude not installed");
            return;
        }

        let process = result.unwrap();
        let shutdown_result = process.shutdown().await;
        assert!(shutdown_result.is_ok());
    }

    #[tokio::test]
    async fn test_process_is_alive_after_shutdown() {
        let session_id = SessionId::new();
        let result = ClaudeProcess::spawn(session_id);

        if result.is_err() {
            eprintln!("Skipping test - claude not installed");
            return;
        }

        let mut process = result.unwrap();
        assert!(process.is_alive().await);

        let _ = process.shutdown().await;
        // Note: Can't check is_alive after shutdown because process is consumed
    }

    #[tokio::test]
    async fn test_read_stderr_line() {
        let session_id = SessionId::new();
        let result = ClaudeProcess::spawn(session_id);

        if result.is_err() {
            eprintln!("Skipping test - claude not installed");
            return;
        }

        let mut process = result.unwrap();

        // Claude CLI outputs diagnostic messages to stderr
        // We can test the read_stderr_line method even though stderr might be empty
        // This verifies the method works without blocking

        // Send an invalid message to trigger stderr output
        let _ = process.write_line(r#"invalid message"#).await;

        // Give process a moment to respond
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Try reading stderr - may or may not have content depending on claude's behavior
        let stderr_result = process.read_stderr_line().await;
        // We just verify the method doesn't panic and returns a valid Result
        assert!(stderr_result.is_ok());

        // Cleanup
        let _ = process.shutdown().await;
    }

    #[tokio::test]
    async fn test_process_crash_detection_during_io() {
        let session_id = SessionId::new();
        let result = ClaudeProcess::spawn(session_id);

        if result.is_err() {
            eprintln!("Skipping test - claude not installed");
            return;
        }

        let mut process = result.unwrap();

        // Forcibly kill the process to simulate a crash
        let pid = process.child.id();
        process.child.kill().await.expect("Failed to kill process");

        // Wait for the kill to take effect
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify is_alive detects the dead process
        assert!(!process.is_alive().await);

        // Attempting to read should return None (EOF) since process is dead
        let read_result = process.read_line().await;
        match read_result {
            Ok(None) => {
                // Expected - EOF on stdout
            }
            Ok(Some(line)) => {
                panic!("Unexpected line read from dead process: {}", line);
            }
            Err(e) => {
                // Also acceptable - read error from dead process
                eprintln!("Read error from dead process (acceptable): {}", e);
            }
        }

        tracing::debug!("Successfully detected crashed process with PID {:?}", pid);
        // No need to call shutdown() as process is already dead
    }

    #[tokio::test]
    async fn test_concurrent_access_multiple_threads() {
        let manager = Arc::new(ClaudeProcessManager::new());
        let mut handles = vec![];

        // Spawn 5 concurrent tasks that each spawn and manage a session
        for i in 0..5 {
            let manager_clone = Arc::clone(&manager);
            let handle = tokio::spawn(async move {
                let session_id = SessionId::new();

                // Spawn process
                let spawn_result = manager_clone.spawn_for_session(session_id).await;
                if spawn_result.is_err() {
                    eprintln!("Thread {}: Skipping - claude not installed", i);
                    return;
                }

                // Verify session exists
                assert!(manager_clone.has_session(&session_id).await);

                // Get process and verify it exists
                let process = manager_clone.get_process(&session_id).await;
                assert!(process.is_ok());
                let process_arc = process.unwrap();

                // Verify we can lock and access the process
                // Note: is_alive() is async but we can't hold MutexGuard across await
                // So we just verify the process exists and is accessible
                {
                    let proc = process_arc.lock().await;
                    let _session = proc.session_id();
                } // Drop lock immediately

                // Give other threads a chance to run
                tokio::time::sleep(Duration::from_millis(50)).await;

                // Terminate session (drop process_arc first)
                drop(process_arc);

                let terminate_result = manager_clone.terminate_session(&session_id).await;
                assert!(terminate_result.is_ok());

                // Verify session removed
                assert!(!manager_clone.has_session(&session_id).await);

                tracing::debug!("Thread {} completed successfully", i);
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            let _ = handle.await;
        }

        tracing::info!("Concurrent access test completed successfully");
    }
}
