//! User prompt handling for permission requests
//!
//! This module provides mechanisms for prompting users for permission decisions
//! when the permission policy requires user consent.

use async_trait::async_trait;
use std::io::Write;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::tools::PermissionOption;

/// Result type for user prompt operations
pub type PromptResult<T> = Result<T, PromptError>;

/// Errors that can occur during user prompting
#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    /// IO error occurred while reading user input
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// User provided invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Operation timed out
    #[error("Prompt timed out")]
    Timeout,
}

/// Trait for handling user prompts for permission requests
#[async_trait]
pub trait UserPromptHandler: Send + Sync {
    /// Prompt the user to select a permission option
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool requiring permission
    /// * `description` - Human-readable description of the tool action
    /// * `options` - Available permission options
    ///
    /// # Returns
    /// The selected option ID, or an error if the prompt failed
    async fn prompt_for_permission(
        &self,
        tool_name: &str,
        description: &str,
        options: &[PermissionOption],
    ) -> PromptResult<String>;
}

/// Console-based user prompt handler that reads from stdin
pub struct ConsolePromptHandler;

impl ConsolePromptHandler {
    /// Create a new console prompt handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsolePromptHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UserPromptHandler for ConsolePromptHandler {
    async fn prompt_for_permission(
        &self,
        tool_name: &str,
        description: &str,
        options: &[PermissionOption],
    ) -> PromptResult<String> {
        // Print the permission request header
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🔐 PERMISSION REQUEST");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("\nTool: {}", tool_name);
        println!("Action: {}", description);
        println!("\nAvailable options:");

        // Print numbered options
        for (idx, option) in options.iter().enumerate() {
            println!("  {}. {} - {:?}", idx + 1, option.name, option.kind);
        }

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Prompt for input
        print!("Enter your choice (1-{}): ", options.len());
        std::io::stdout().flush()?;

        // Read user input asynchronously
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        // Parse the selection
        let trimmed = line.trim();
        let selection = trimmed
            .parse::<usize>()
            .map_err(|_| PromptError::InvalidInput(format!("Not a valid number: {}", trimmed)))?;

        if selection < 1 || selection > options.len() {
            return Err(PromptError::InvalidInput(format!(
                "Selection {} is out of range (1-{})",
                selection,
                options.len()
            )));
        }

        // Return the selected option ID
        let selected_option = &options[selection - 1];
        println!("✓ Selected: {}\n", selected_option.name);

        Ok(selected_option.option_id.clone())
    }
}

/// Mock prompt handler for testing that always returns a specific option
#[cfg(test)]
pub struct MockPromptHandler {
    response: Option<String>,
}

#[cfg(test)]
impl MockPromptHandler {
    pub fn new(response: Option<String>) -> Self {
        Self { response }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::PermissionOptionKind;

    #[async_trait]
    impl UserPromptHandler for MockPromptHandler {
        async fn prompt_for_permission(
            &self,
            _tool_name: &str,
            _description: &str,
            options: &[PermissionOption],
        ) -> PromptResult<String> {
            match &self.response {
                Some(option_id) => Ok(option_id.clone()),
                None => {
                    // Return first option by default
                    Ok(options
                        .first()
                        .ok_or_else(|| PromptError::InvalidInput("No options available".into()))?
                        .option_id
                        .clone())
                }
            }
        }
    }

    #[tokio::test]
    async fn test_mock_prompt_handler() {
        let options = vec![
            PermissionOption {
                option_id: "allow-once".to_string(),
                name: "Allow Once".to_string(),
                kind: PermissionOptionKind::AllowOnce,
            },
            PermissionOption {
                option_id: "reject-once".to_string(),
                name: "Reject Once".to_string(),
                kind: PermissionOptionKind::RejectOnce,
            },
        ];

        let handler = MockPromptHandler::new(Some("allow-once".to_string()));
        let result = handler
            .prompt_for_permission("test_tool", "Test action", &options)
            .await
            .unwrap();

        assert_eq!(result, "allow-once");
    }

    #[tokio::test]
    async fn test_mock_prompt_handler_default_response() {
        let options = vec![PermissionOption {
            option_id: "allow-once".to_string(),
            name: "Allow Once".to_string(),
            kind: PermissionOptionKind::AllowOnce,
        }];

        let handler = MockPromptHandler::new(None);
        let result = handler
            .prompt_for_permission("test_tool", "Test action", &options)
            .await
            .unwrap();

        assert_eq!(result, "allow-once");
    }
}
