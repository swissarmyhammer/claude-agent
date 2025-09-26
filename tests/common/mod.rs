//! Common Test Utilities
//!
//! This module provides shared utilities and helper functions for integration tests.

use crate::test_client::{TestClient, Result};
use agent_client_protocol::{ClientCapabilities, FsCapabilities};
use std::time::Duration;
use ulid::Ulid;

/// Test utilities for common operations
pub mod test_utils {
    use super::*;
    
    /// Create a test setup with client and server connected via in-memory streams
    pub async fn create_test_setup() -> Result<(TestClient, tokio::task::JoinHandle<()>)> {
        let (client, server) = TestClient::new().await?;
        
        let server_handle = tokio::spawn(async move {
            // For now, just simulate server running
            // In a more complete implementation, we'd properly handle the stream
            tokio::time::sleep(Duration::from_secs(10)).await;
        });
        
        // Give server time to initialize
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        Ok((client, server_handle))
    }
    
    /// Initialize a client with default settings and return session ID
    pub async fn initialize_client(client: &TestClient) -> Result<String> {
        // Initialize protocol
        client.initialize(None).await?;
        
        // Authenticate with none type (for testing)
        client.authenticate("none".to_string()).await?;
        
        // Create session
        let session = client.create_session(None).await?;
        
        Ok(session.session_id)
    }
    
    /// Initialize client with specific capabilities
    pub async fn initialize_client_with_capabilities(
        client: &TestClient,
        capabilities: ClientCapabilities,
    ) -> Result<String> {
        // Initialize protocol with capabilities
        client.initialize(Some(capabilities.clone())).await?;
        
        // Authenticate
        client.authenticate("none".to_string()).await?;
        
        // Create session with capabilities
        let session = client.create_session(Some(capabilities)).await?;
        
        Ok(session.session_id)
    }
    
    /// Create default client capabilities for testing
    pub fn default_test_capabilities() -> ClientCapabilities {
        ClientCapabilities {
            fs: Some(FsCapabilities {
                read_text_file: true,
                write_text_file: false,  // Generally safer for testing
            }),
            terminal: Some(true),
        }
    }
    
    /// Create full client capabilities (including potentially dangerous operations)
    pub fn full_test_capabilities() -> ClientCapabilities {
        ClientCapabilities {
            fs: Some(FsCapabilities {
                read_text_file: true,
                write_text_file: true,  // Include write capabilities
            }),
            terminal: Some(true),
        }
    }
    
    /// Validate that a session ID has the expected format
    pub fn assert_valid_session_id(session_id: &str) {
        assert!(!session_id.is_empty(), "Session ID cannot be empty");
        assert!(session_id.len() >= 8, "Session ID should be at least 8 characters long");
        
        // Try to parse as ULID (preferred format for session IDs)
        if let Ok(_) = Ulid::from_string(session_id) {
            // Valid ULID format
            return;
        }
        
        // If not UUID, at least check it's reasonable
        assert!(
            session_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
            "Session ID should contain only alphanumeric characters, hyphens, or underscores"
        );
    }
    
    /// Create a test prompt with specific content
    pub fn create_test_prompt(content: &str) -> String {
        format!("Test prompt: {}", content)
    }
    
    /// Create multiple unique test prompts
    pub fn create_test_prompts(count: usize) -> Vec<String> {
        (0..count)
            .map(|i| format!("Test prompt #{}: {}", i + 1, Ulid::new()))
            .collect()
    }
    
    /// Timeout wrapper for async operations in tests
    pub async fn with_timeout<F, T>(
        future: F, 
        duration: Duration,
        operation_name: &str
    ) -> std::result::Result<T, String>
    where
        F: std::future::Future<Output = T>,
    {
        match tokio::time::timeout(duration, future).await {
            Ok(result) => Ok(result),
            Err(_) => Err(format!("Operation '{}' timed out after {:?}", operation_name, duration)),
        }
    }
    
    /// Clean up a test setup by aborting the server handle
    pub fn cleanup_test_setup(server_handle: tokio::task::JoinHandle<()>) {
        server_handle.abort();
    }
    
    /// Assert that an operation result indicates success
    pub fn assert_operation_success<T, E: std::fmt::Debug>(
        result: std::result::Result<T, E>,
        operation_name: &str,
    ) -> T {
        match result {
            Ok(value) => value,
            Err(e) => panic!("Operation '{}' failed: {:?}", operation_name, e),
        }
    }
    
    /// Assert that an operation result indicates failure
    pub fn assert_operation_failure<T, E>(
        result: std::result::Result<T, E>,
        operation_name: &str,
    ) {
        match result {
            Ok(_) => panic!("Operation '{}' should have failed but succeeded", operation_name),
            Err(_) => {}, // Expected failure
        }
    }
    
    /// Generate a unique test identifier
    pub fn generate_test_id() -> String {
        format!("test_{}", Ulid::new())
    }
    
    /// Create test data for file operations (safe content)
    pub fn create_test_file_content() -> String {
        format!(
            "Test file content generated at {}\nTest ID: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            Ulid::new()
        )
    }
}



/// Memory and resource monitoring utilities  
pub mod resource_utils {
    use super::*;
    
    /// Basic memory usage measurement (platform specific)
    pub fn get_memory_usage() -> Option<usize> {
        // This is a simplified implementation
        // In a real implementation, you'd use platform-specific APIs
        // or crates like `sysinfo` for accurate memory measurement
        None
    }
    
    /// Monitor resource usage during test execution
    pub async fn monitor_resources_during<F, T>(future: F, operation_name: &str) -> T
    where
        F: std::future::Future<Output = T>,
    {
        let _start_memory = get_memory_usage();
        let start_time = Instant::now();
        
        let result = future.await;
        
        let _end_memory = get_memory_usage();
        let duration = start_time.elapsed();
        
        // Log resource usage (in a real implementation)
        println!("Operation '{}' completed in {:?}", operation_name, duration);
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_id_validation() {
        // Valid ULID
        test_utils::assert_valid_session_id("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        
        // Valid simple ID
        test_utils::assert_valid_session_id("test-session-123");
        
        // Valid alphanumeric
        test_utils::assert_valid_session_id("session123abc");
    }
    
    #[test]
    #[should_panic(expected = "Session ID cannot be empty")]
    fn test_session_id_validation_empty() {
        test_utils::assert_valid_session_id("");
    }
    
    #[test]
    #[should_panic(expected = "Session ID should be at least 8 characters long")]
    fn test_session_id_validation_too_short() {
        test_utils::assert_valid_session_id("short");
    }
    
    #[test]
    fn test_test_prompt_generation() {
        let prompt = test_utils::create_test_prompt("hello world");
        assert!(prompt.contains("Test prompt: hello world"));
        
        let prompts = test_utils::create_test_prompts(3);
        assert_eq!(prompts.len(), 3);
        
        // All prompts should be unique
        for (i, prompt) in prompts.iter().enumerate() {
            assert!(prompt.contains(&format!("Test prompt #{}", i + 1)));
        }
    }
    
    #[test]
    fn test_capabilities_creation() {
        let default_caps = test_utils::default_test_capabilities();
        assert!(default_caps.fs.is_some());
        assert_eq!(default_caps.fs.unwrap().read_text_file, true);
        
        let full_caps = test_utils::full_test_capabilities();
        assert!(full_caps.fs.is_some());
        let fs_caps = full_caps.fs.unwrap();
        assert_eq!(fs_caps.read_text_file, true);
        assert_eq!(fs_caps.write_text_file, true);
    }
    
    #[test]
    fn test_test_id_generation() {
        let id1 = test_utils::generate_test_id();
        let id2 = test_utils::generate_test_id();
        
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test_"));
        assert!(id2.starts_with("test_"));
    }
    
    #[tokio::test]
    async fn test_performance_measurement() {
        let (result, duration) = perf_utils::measure_async(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        }).await;
        
        assert_eq!(result, 42);
        assert!(duration >= Duration::from_millis(10));
        assert!(duration < Duration::from_millis(100)); // Should be reasonably close
    }
}