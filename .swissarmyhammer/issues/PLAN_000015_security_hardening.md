# Security Hardening

Refer to plan.md

## Goal
Implement comprehensive security features including path validation, sandboxing, audit logging, and security testing.

## Tasks

### 1. Enhanced Path Validation (`lib/src/security.rs`)

```rust
use std::path::{Path, PathBuf};
use regex::Regex;

pub struct PathValidator {
    allowed_patterns: Vec<Regex>,
    forbidden_paths: Vec<PathBuf>,
    sandbox_root: Option<PathBuf>,
}

impl PathValidator {
    pub fn new(config: &crate::config::SecurityConfig) -> crate::Result<Self> {
        let allowed_patterns = config.allowed_file_patterns
            .iter()
            .map(|pattern| {
                // Convert glob pattern to regex
                let regex_pattern = pattern
                    .replace("**", ".*")
                    .replace("*", "[^/]*")
                    .replace("?", "[^/]");
                Regex::new(&format!("^{}$", regex_pattern))
                    .map_err(|e| crate::AgentError::Config(format!("Invalid pattern {}: {}", pattern, e)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        let forbidden_paths = config.forbidden_paths
            .iter()
            .map(PathBuf::from)
            .collect();
        
        Ok(Self {
            allowed_patterns,
            forbidden_paths,
            sandbox_root: None,
        })
    }
    
    pub fn with_sandbox_root(mut self, root: PathBuf) -> Self {
        self.sandbox_root = Some(root);
        self
    }
    
    pub fn validate_path(&self, path: &str, operation: PathOperation) -> crate::Result<PathBuf> {
        let path_buf = PathBuf::from(path);
        
        // Validate against path traversal attacks
        self.check_path_traversal(&path_buf)?;
        
        // Validate against null bytes and other malicious patterns
        self.check_malicious_patterns(path)?;
        
        // Resolve to canonical path
        let canonical_path = self.resolve_path(&path_buf)?;
        
        // Check sandbox constraints
        if let Some(ref sandbox_root) = self.sandbox_root {
            self.check_sandbox_constraints(&canonical_path, sandbox_root)?;
        }
        
        // Check forbidden paths
        self.check_forbidden_paths(&canonical_path)?;
        
        // Check allowed patterns
        self.check_allowed_patterns(&canonical_path, operation)?;
        
        Ok(canonical_path)
    }
    
    fn check_path_traversal(&self, path: &Path) -> crate::Result<()> {
        let path_str = path.to_string_lossy();
        
        // Check for directory traversal patterns
        let dangerous_patterns = [
            "..",
            "~",
            "/./",
            "/../",
            "\\..\\",
            "\\.\\",
        ];
        
        for pattern in &dangerous_patterns {
            if path_str.contains(pattern) {
                return Err(crate::AgentError::PermissionDenied(
                    format!("Path contains dangerous pattern '{}': {}", pattern, path_str)
                ));
            }
        }
        
        Ok(())
    }
    
    fn check_malicious_patterns(&self, path: &str) -> crate::Result<()> {
        // Check for null bytes
        if path.contains('\0') {
            return Err(crate::AgentError::PermissionDenied(
                "Path contains null bytes".to_string()
            ));
        }
        
        // Check for extremely long paths
        if path.len() > 4096 {
            return Err(crate::AgentError::PermissionDenied(
                "Path too long".to_string()
            ));
        }
        
        // Check for suspicious patterns
        let suspicious_patterns = [
            "/proc/",
            "/sys/",
            "/dev/",
            "\\\\.\\",  // Windows device paths
            "CON",     // Windows reserved names
            "PRN",
            "AUX",
            "NUL",
        ];
        
        for pattern in &suspicious_patterns {
            if path.contains(pattern) {
                return Err(crate::AgentError::PermissionDenied(
                    format!("Path contains suspicious pattern '{}': {}", pattern, path)
                ));
            }
        }
        
        Ok(())
    }
    
    fn resolve_path(&self, path: &Path) -> crate::Result<PathBuf> {
        // Convert to absolute path
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| crate::AgentError::Io(e))?
                .join(path)
        };
        
        // Canonicalize if path exists, otherwise validate parent
        match absolute_path.canonicalize() {
            Ok(canonical) => Ok(canonical),
            Err(_) => {
                // If path doesn't exist, validate parent directory
                if let Some(parent) = absolute_path.parent() {
                    if parent.exists() {
                        let canonical_parent = parent.canonicalize()
                            .map_err(|e| crate::AgentError::PermissionDenied(
                                format!("Cannot resolve parent directory: {}", e)
                            ))?;
                        
                        if let Some(filename) = absolute_path.file_name() {
                            Ok(canonical_parent.join(filename))
                        } else {
                            Err(crate::AgentError::PermissionDenied(
                                "Invalid file path".to_string()
                            ))
                        }
                    } else {
                        Err(crate::AgentError::PermissionDenied(
                            "Parent directory does not exist".to_string()
                        ))
                    }
                } else {
                    Err(crate::AgentError::PermissionDenied(
                        "Invalid path structure".to_string()
                    ))
                }
            }
        }
    }
    
    fn check_sandbox_constraints(&self, path: &Path, sandbox_root: &Path) -> crate::Result<()> {
        if !path.starts_with(sandbox_root) {
            return Err(crate::AgentError::PermissionDenied(
                format!("Path outside sandbox: {} (sandbox root: {})", 
                       path.display(), 
                       sandbox_root.display())
            ));
        }
        Ok(())
    }
    
    fn check_forbidden_paths(&self, path: &Path) -> crate::Result<()> {
        for forbidden in &self.forbidden_paths {
            if path.starts_with(forbidden) {
                return Err(crate::AgentError::PermissionDenied(
                    format!("Access forbidden to path: {}", path.display())
                ));
            }
        }
        Ok(())
    }
    
    fn check_allowed_patterns(&self, path: &Path, operation: PathOperation) -> crate::Result<()> {
        if self.allowed_patterns.is_empty() {
            return Ok(()); // No restrictions if no patterns defined
        }
        
        let path_str = path.to_string_lossy();
        
        for pattern in &self.allowed_patterns {
            if pattern.is_match(&path_str) {
                return Ok(());
            }
        }
        
        Err(crate::AgentError::PermissionDenied(
            format!("Path does not match any allowed pattern: {} (operation: {:?})", 
                   path_str, operation)
        ))
    }
}

#[derive(Debug, Clone)]
pub enum PathOperation {
    Read,
    Write,
    Execute,
    List,
}
```

### 2. Audit Logging System

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub session_id: Option<String>,
    pub event_type: AuditEventType,
    pub details: serde_json::Value,
    pub user_agent: Option<String>,
    pub result: AuditResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    SessionCreated,
    AuthenticationAttempt,
    ToolCallRequested,
    ToolCallExecuted,
    FileAccessed,
    TerminalCommandExecuted,
    PermissionGranted,
    PermissionDenied,
    SecurityViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditResult {
    Success,
    Failure(String),
    Denied(String),
}

pub struct AuditLogger {
    sender: mpsc::UnboundedSender<AuditEvent>,
}

impl AuditLogger {
    pub fn new() -> (Self, AuditLogWriter) {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        let writer = AuditLogWriter::new(receiver);
        let logger = Self { sender };
        
        (logger, writer)
    }
    
    pub fn log_event(&self, event: AuditEvent) {
        if let Err(_) = self.sender.send(event.clone()) {
            error!("Failed to send audit event to logger");
            // Fall back to direct logging
            warn!("AUDIT: {:?}", event);
        }
    }
    
    pub fn log_session_created(&self, session_id: &str) {
        let event = AuditEvent {
            timestamp: chrono::Utc::now(),
            session_id: Some(session_id.to_string()),
            event_type: AuditEventType::SessionCreated,
            details: serde_json::json!({}),
            user_agent: None,
            result: AuditResult::Success,
        };
        self.log_event(event);
    }
    
    pub fn log_tool_call(&self, session_id: &str, tool_name: &str, result: &crate::Result<()>) {
        let (event_type, audit_result) = match result {
            Ok(_) => (AuditEventType::ToolCallExecuted, AuditResult::Success),
            Err(e) => (AuditEventType::ToolCallRequested, AuditResult::Failure(e.to_string())),
        };
        
        let event = AuditEvent {
            timestamp: chrono::Utc::now(),
            session_id: Some(session_id.to_string()),
            event_type,
            details: serde_json::json!({
                "tool_name": tool_name
            }),
            user_agent: None,
            result: audit_result,
        };
        self.log_event(event);
    }
    
    pub fn log_file_access(&self, session_id: &str, path: &str, operation: PathOperation, result: &crate::Result<()>) {
        let audit_result = match result {
            Ok(_) => AuditResult::Success,
            Err(e) => AuditResult::Failure(e.to_string()),
        };
        
        let event = AuditEvent {
            timestamp: chrono::Utc::now(),
            session_id: Some(session_id.to_string()),
            event_type: AuditEventType::FileAccessed,
            details: serde_json::json!({
                "path": path,
                "operation": format!("{:?}", operation)
            }),
            user_agent: None,
            result: audit_result,
        };
        self.log_event(event);
    }
    
    pub fn log_security_violation(&self, session_id: Option<&str>, violation: &str, details: serde_json::Value) {
        let event = AuditEvent {
            timestamp: chrono::Utc::now(),
            session_id: session_id.map(String::from),
            event_type: AuditEventType::SecurityViolation,
            details,
            user_agent: None,
            result: AuditResult::Denied(violation.to_string()),
        };
        self.log_event(event);
    }
}

pub struct AuditLogWriter {
    receiver: mpsc::UnboundedReceiver<AuditEvent>,
}

impl AuditLogWriter {
    fn new(receiver: mpsc::UnboundedReceiver<AuditEvent>) -> Self {
        Self { receiver }
    }
    
    pub async fn start(mut self) {
        while let Some(event) = self.receiver.recv().await {
            self.write_event(&event).await;
        }
    }
    
    async fn write_event(&self, event: &AuditEvent) {
        // Write to structured log
        info!(
            target: "audit",
            timestamp = %event.timestamp,
            session_id = ?event.session_id,
            event_type = ?event.event_type,
            result = ?event.result,
            details = %event.details,
            "AUDIT EVENT"
        );
        
        // Could also write to database, file, or external audit system
        self.write_to_file(event).await.unwrap_or_else(|e| {
            error!("Failed to write audit event to file: {}", e);
        });
    }
    
    async fn write_to_file(&self, event: &AuditEvent) -> Result<(), std::io::Error> {
        use tokio::fs::OpenOptions;
        use tokio::io::AsyncWriteExt;
        
        let log_line = format!("{}\n", serde_json::to_string(event).unwrap_or_default());
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("audit.log")
            .await?;
        
        file.write_all(log_line.as_bytes()).await?;
        file.flush().await?;
        
        Ok(())
    }
}
```

### 3. Command Validation and Sanitization

```rust
pub struct CommandValidator {
    forbidden_commands: Vec<String>,
    forbidden_patterns: Vec<Regex>,
    max_command_length: usize,
}

impl CommandValidator {
    pub fn new() -> Self {
        let forbidden_commands = vec![
            "rm".to_string(),
            "rmdir".to_string(),
            "del".to_string(),
            "format".to_string(),
            "fdisk".to_string(),
            "mkfs".to_string(),
            "dd".to_string(),
            "shutdown".to_string(),
            "reboot".to_string(),
            "halt".to_string(),
            "poweroff".to_string(),
            "init".to_string(),
            "kill".to_string(),
            "killall".to_string(),
            "pkill".to_string(),
            "crontab".to_string(),
            "at".to_string(),
            "chmod".to_string(),
            "chown".to_string(),
            "chgrp".to_string(),
            "mount".to_string(),
            "umount".to_string(),
            "su".to_string(),
            "sudo".to_string(),
        ];
        
        let forbidden_patterns = vec![
            Regex::new(r"rm\s+-rf\s+/").unwrap(),
            Regex::new(r">\s*/dev/").unwrap(),
            Regex::new(r"\|\s*sh\s*$").unwrap(),
            Regex::new(r"\|\s*bash\s*$").unwrap(),
            Regex::new(r"curl\s+.*\s*\|\s*sh").unwrap(),
            Regex::new(r"wget\s+.*\s*\|\s*sh").unwrap(),
            Regex::new(r"eval\s+").unwrap(),
            Regex::new(r"exec\s+").unwrap(),
            Regex::new(r"system\s*\(").unwrap(),
        ];
        
        Self {
            forbidden_commands,
            forbidden_patterns,
            max_command_length: 1000,
        }
    }
    
    pub fn validate_command(&self, command: &str) -> crate::Result<()> {
        let trimmed = command.trim();
        
        // Check command length
        if trimmed.len() > self.max_command_length {
            return Err(crate::AgentError::PermissionDenied(
                format!("Command too long: {} characters (max: {})", 
                       trimmed.len(), self.max_command_length)
            ));
        }
        
        // Check for empty commands
        if trimmed.is_empty() {
            return Err(crate::AgentError::PermissionDenied(
                "Empty command not allowed".to_string()
            ));
        }
        
        // Extract the main command (first word)
        let main_command = trimmed.split_whitespace().next().unwrap_or("");
        let command_name = std::path::Path::new(main_command)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(main_command);
        
        // Check forbidden commands
        if self.forbidden_commands.contains(&command_name.to_string()) {
            return Err(crate::AgentError::PermissionDenied(
                format!("Command '{}' is forbidden", command_name)
            ));
        }
        
        // Check forbidden patterns
        for pattern in &self.forbidden_patterns {
            if pattern.is_match(trimmed) {
                return Err(crate::AgentError::PermissionDenied(
                    "Command contains forbidden pattern".to_string()
                ));
            }
        }
        
        // Check for suspicious shell features
        let suspicious_chars = ['|', '&', ';', '`', '$', '>', '<', '(', ')', '{', '}'];
        let suspicious_count = trimmed.chars()
            .filter(|c| suspicious_chars.contains(c))
            .count();
        
        if suspicious_count > 3 {
            return Err(crate::AgentError::PermissionDenied(
                "Command contains too many shell metacharacters".to_string()
            ));
        }
        
        Ok(())
    }
    
    pub fn sanitize_arguments(&self, args: &[String]) -> Vec<String> {
        args.iter()
            .map(|arg| {
                // Remove null bytes
                let clean_arg = arg.replace('\0', "");
                
                // Limit argument length
                if clean_arg.len() > 256 {
                    clean_arg[..256].to_string()
                } else {
                    clean_arg
                }
            })
            .collect()
    }
}
```

### 4. Rate Limiting

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct RateLimiter {
    limits: Arc<RwLock<HashMap<String, RateLimit>>>,
    max_requests_per_minute: u32,
    max_requests_per_hour: u32,
}

#[derive(Debug, Clone)]
struct RateLimit {
    requests_this_minute: u32,
    requests_this_hour: u32,
    minute_start: Instant,
    hour_start: Instant,
}

impl RateLimiter {
    pub fn new(max_per_minute: u32, max_per_hour: u32) -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
            max_requests_per_minute: max_per_minute,
            max_requests_per_hour: max_per_hour,
        }
    }
    
    pub async fn check_rate_limit(&self, session_id: &str) -> crate::Result<()> {
        let mut limits = self.limits.write().await;
        let now = Instant::now();
        
        let rate_limit = limits.entry(session_id.to_string())
            .or_insert_with(|| RateLimit {
                requests_this_minute: 0,
                requests_this_hour: 0,
                minute_start: now,
                hour_start: now,
            });
        
        // Reset counters if time windows have passed
        if now.duration_since(rate_limit.minute_start) >= Duration::from_secs(60) {
            rate_limit.requests_this_minute = 0;
            rate_limit.minute_start = now;
        }
        
        if now.duration_since(rate_limit.hour_start) >= Duration::from_secs(3600) {
            rate_limit.requests_this_hour = 0;
            rate_limit.hour_start = now;
        }
        
        // Check limits
        if rate_limit.requests_this_minute >= self.max_requests_per_minute {
            return Err(crate::AgentError::PermissionDenied(
                "Rate limit exceeded: too many requests per minute".to_string()
            ));
        }
        
        if rate_limit.requests_this_hour >= self.max_requests_per_hour {
            return Err(crate::AgentError::PermissionDenied(
                "Rate limit exceeded: too many requests per hour".to_string()
            ));
        }
        
        // Increment counters
        rate_limit.requests_this_minute += 1;
        rate_limit.requests_this_hour += 1;
        
        Ok(())
    }
    
    pub async fn cleanup_expired(&self) {
        let mut limits = self.limits.write().await;
        let now = Instant::now();
        
        limits.retain(|_, rate_limit| {
            // Keep entries that have recent activity
            now.duration_since(rate_limit.hour_start) < Duration::from_secs(3600 * 2)
        });
    }
}
```

### 5. Security Integration with Agent

```rust
// In lib/src/agent.rs - integrate security components

impl ClaudeAgent {
    pub async fn new(config: AgentConfig) -> crate::Result<(Self, broadcast::Receiver<SessionUpdateNotification>)> {
        // ... existing initialization ...
        
        // Initialize security components
        let path_validator = Arc::new(PathValidator::new(&config.security)?);
        let command_validator = Arc::new(CommandValidator::new());
        let rate_limiter = Arc::new(RateLimiter::new(100, 1000)); // 100/min, 1000/hour
        
        let (audit_logger, audit_writer) = AuditLogger::new();
        let audit_logger = Arc::new(audit_logger);
        
        // Start audit log writer
        tokio::spawn(audit_writer.start());
        
        let agent = Self {
            // ... existing fields ...
            path_validator: Some(path_validator),
            command_validator: Some(command_validator),
            rate_limiter: Some(rate_limiter),
            audit_logger: Some(audit_logger),
        };
        
        Ok((agent, notification_receiver))
    }
    
    async fn session_prompt(&self, request: PromptRequest) -> crate::Result<PromptResponse> {
        // Check rate limit first
        if let Some(ref rate_limiter) = self.rate_limiter {
            rate_limiter.check_rate_limit(&request.session_id).await?;
        }
        
        // Log the prompt attempt
        if let Some(ref audit_logger) = self.audit_logger {
            audit_logger.log_event(AuditEvent {
                timestamp: chrono::Utc::now(),
                session_id: Some(request.session_id.clone()),
                event_type: AuditEventType::ToolCallRequested,
                details: serde_json::json!({
                    "prompt_length": request.prompt.len()
                }),
                user_agent: None,
                result: AuditResult::Success,
            });
        }
        
        // ... existing prompt handling logic ...
    }
}
```

### 6. Security Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_path_validation() {
        let config = crate::config::SecurityConfig {
            allowed_file_patterns: vec!["**/*.txt".to_string()],
            forbidden_paths: vec!["/etc".to_string()],
            require_permission_for: vec![],
        };
        
        let validator = PathValidator::new(&config).unwrap();
        
        // Test valid path
        let result = validator.validate_path("test.txt", PathOperation::Read);
        assert!(result.is_ok());
        
        // Test path traversal
        let result = validator.validate_path("../../../etc/passwd", PathOperation::Read);
        assert!(result.is_err());
        
        // Test forbidden path
        let result = validator.validate_path("/etc/hosts", PathOperation::Read);
        assert!(result.is_err());
        
        // Test pattern mismatch
        let result = validator.validate_path("test.exe", PathOperation::Read);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_command_validation() {
        let validator = CommandValidator::new();
        
        // Test safe command
        let result = validator.validate_command("ls -la");
        assert!(result.is_ok());
        
        // Test dangerous command
        let result = validator.validate_command("rm -rf /");
        assert!(result.is_err());
        
        // Test command with shell injection
        let result = validator.validate_command("ls; rm -rf /");
        assert!(result.is_err());
        
        // Test command too long
        let long_command = "a".repeat(2000);
        let result = validator.validate_command(&long_command);
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_rate_limiting() {
        let rate_limiter = RateLimiter::new(2, 10);
        
        // First two requests should succeed
        assert!(rate_limiter.check_rate_limit("session1").await.is_ok());
        assert!(rate_limiter.check_rate_limit("session1").await.is_ok());
        
        // Third request should fail (exceeds per-minute limit)
        assert!(rate_limiter.check_rate_limit("session1").await.is_err());
        
        // Different session should still work
        assert!(rate_limiter.check_rate_limit("session2").await.is_ok());
    }
    
    #[tokio::test]
    async fn test_audit_logging() {
        let (audit_logger, _writer) = AuditLogger::new();
        
        // Test logging doesn't panic
        audit_logger.log_session_created("test-session");
        audit_logger.log_file_access("test-session", "test.txt", PathOperation::Read, &Ok(()));
        audit_logger.log_security_violation(Some("test-session"), "test violation", json!({}));
        
        // In a real test, we'd verify the logs were written correctly
    }
}
```

## Files Created
- `lib/src/security.rs` - Comprehensive security validation and audit logging
- Update `lib/src/agent.rs` - Integrate security components
- Update `lib/src/tools.rs` - Add security validation to tool execution
- Add comprehensive security tests

## Dependencies
Add to `lib/Cargo.toml`:
```toml
[dependencies]
# ... existing dependencies ...
regex = "1.10"
chrono = { version = "0.4", features = ["serde"] }
```

## Acceptance Criteria
- Path validation prevents directory traversal and other attacks
- Command validation blocks dangerous commands and patterns
- Audit logging captures all security-relevant events
- Rate limiting prevents abuse
- Security violations are properly logged and blocked
- Integration with agent maintains security throughout request flow
- Security tests cover major attack vectors
- Performance impact of security measures is minimal
- Configuration allows customization of security policies
- `cargo build` and `cargo test` succeed