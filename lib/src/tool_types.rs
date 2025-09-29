//! Tool call data structures and types for ACP compliance
//!
//! This module contains the core data structures used for tool call reporting
//! according to the Agent Client Protocol (ACP) specification.

use serde::{Deserialize, Serialize};

/// ACP-compliant tool call classification according to specification
///
/// Tool kinds help Clients choose appropriate icons and optimize how they display
/// tool execution progress. This enum matches the ACP specification exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// Reading files or data
    Read,
    /// Modifying files or content  
    Edit,
    /// Removing files or data
    Delete,
    /// Moving or renaming files
    Move,
    /// Searching for information
    Search,
    /// Running commands or code
    Execute,
    /// Internal reasoning or planning
    Think,
    /// Retrieving external data
    Fetch,
    /// Other tool types (default)
    #[serde(other)]
    Other,
}

/// ACP-compliant tool call execution status
///
/// Tool calls progress through different statuses during their lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    /// The tool call hasn't started running yet because the input is either streaming or awaiting approval
    Pending,
    /// The tool call is currently running
    InProgress,
    /// The tool call completed successfully
    Completed,
    /// The tool call failed with an error
    Failed,
}

/// Content produced by a tool call execution
///
/// Tool calls can produce different types of content including regular content blocks,
/// file diffs, and embedded terminals for live command output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    /// Standard content blocks like text, images, or resources
    Content {
        /// The actual content block
        content: agent_client_protocol::ContentBlock,
    },
    /// File modifications shown as diffs
    Diff {
        /// The absolute file path being modified
        path: String,
        /// The original content (null for new files)
        #[serde(rename = "oldText")]
        old_text: Option<String>,
        /// The new content after modification
        #[serde(rename = "newText")]
        new_text: String,
    },
    /// Live terminal output from command execution
    Terminal {
        /// The ID of a terminal created with terminal/create
        #[serde(rename = "terminalId")]
        terminal_id: String,
    },
}

/// File location affected by a tool call for "follow-along" features
///
/// Tool calls can report file locations they're working with, enabling Clients 
/// to implement features that track which files the Agent is accessing or modifying.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallLocation {
    /// The absolute file path being accessed or modified
    pub path: String,
    /// Optional line number within the file
    pub line: Option<u64>,
}

/// Complete ACP-compliant tool call report structure
///
/// This struct contains all metadata required by the ACP specification for
/// comprehensive tool call reporting with rich client experiences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallReport {
    /// Unique identifier for this tool call within the session
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    /// Human-readable title describing what the tool is doing
    pub title: String,
    /// The category of tool being invoked
    pub kind: ToolKind,
    /// The current execution status
    pub status: ToolCallStatus,
    /// Content produced by the tool call
    #[serde(default)]
    pub content: Vec<ToolCallContent>,
    /// File locations affected by this tool call
    #[serde(default)]
    pub locations: Vec<ToolCallLocation>,
    /// The raw input parameters sent to the tool
    #[serde(rename = "rawInput", skip_serializing_if = "Option::is_none")]
    pub raw_input: Option<serde_json::Value>,
    /// The raw output returned by the tool
    #[serde(rename = "rawOutput", skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<serde_json::Value>,
}

impl ToolCallReport {
    /// Create a new tool call report
    pub fn new(tool_call_id: String, title: String, kind: ToolKind) -> Self {
        Self {
            tool_call_id,
            title,
            kind,
            status: ToolCallStatus::Pending,
            content: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            raw_output: None,
        }
    }

    /// Update the status of this tool call
    pub fn update_status(&mut self, status: ToolCallStatus) {
        self.status = status;
    }

    /// Add content to this tool call
    pub fn add_content(&mut self, content: ToolCallContent) {
        self.content.push(content);
    }

    /// Add a file location to this tool call
    pub fn add_location(&mut self, location: ToolCallLocation) {
        self.locations.push(location);
    }

    /// Set the raw input parameters for this tool call
    pub fn set_raw_input(&mut self, input: serde_json::Value) {
        self.raw_input = Some(input);
    }

    /// Set the raw output for this tool call
    pub fn set_raw_output(&mut self, output: serde_json::Value) {
        self.raw_output = Some(output);
    }
}