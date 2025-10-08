//! Protocol translator between ACP and Claude CLI stream-json format
//!
//! This module provides translation between the Agent Client Protocol (ACP) message format
//! and the stream-json format used by the claude CLI for stdin/stdout communication.
//!
//! # Stream-JSON Format
//!
//! ## Input (stdin to claude)
//! ```json
//! {"type":"user","message":{"role":"user","content":"What is 2+2?"}}
//! ```
//!
//! ## Output (stdout from claude)
//! ```json
//! {"type":"system","subtype":"init","cwd":"/path","session_id":"uuid","tools":[...]}
//! {"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"read_file","input":{...}}]}}
//! {"type":"result","subtype":"success","total_cost_usd":0.114}
//! ```

use crate::{AgentError, Result};
use agent_client_protocol::{
    ContentBlock, SessionId, SessionNotification, SessionUpdate, TextContent,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Protocol translator for converting between ACP and stream-json formats
pub struct ProtocolTranslator;

impl ProtocolTranslator {
    /// Convert ACP ContentBlocks to stream-json for claude stdin
    ///
    /// Currently only supports single text content blocks. The claude CLI's stream-json
    /// format accepts a simple string for user messages, which limits us to text-only content.
    /// Complex content arrays (images, audio, etc.) would require the full Messages API format
    /// which is not supported by the CLI's stream-json stdin interface.
    ///
    /// # Arguments
    /// * `content` - The content blocks to translate
    ///
    /// # Returns
    /// A JSON string formatted for stream-json input
    ///
    /// # Errors
    /// Returns error if content is not a single text block, or if serialization fails
    pub fn acp_to_stream_json(content: Vec<ContentBlock>) -> Result<String> {
        let content_str = if content.len() == 1 {
            if let ContentBlock::Text(text_content) = &content[0] {
                text_content.text.clone()
            } else {
                return Err(AgentError::Internal(
                    "Only text content blocks are currently supported".to_string(),
                ));
            }
        } else {
            return Err(AgentError::Internal(
                "Only single content blocks are currently supported".to_string(),
            ));
        };

        let message = StreamJsonUserMessage {
            r#type: "user".to_string(),
            message: UserMessage {
                role: "user".to_string(),
                content: content_str,
            },
        };

        serde_json::to_string(&message).map_err(|e| {
            AgentError::Internal(format!("Failed to serialize stream-json message: {}", e))
        })
    }

    /// Convert stream-json line from claude to ACP SessionNotification
    ///
    /// Converts a single line of stream-json output from the claude CLI into an ACP notification.
    /// Note: The claude CLI can output messages with multiple content blocks (e.g., text + tool_use),
    /// but ACP SessionUpdate::AgentMessageChunk only supports a single ContentBlock per notification.
    /// When multiple content items are present, only the first is returned, with a debug log for the rest.
    ///
    /// # Arguments
    /// * `line` - A single line of JSON from claude stdout
    /// * `session_id` - The session ID for the notification
    ///
    /// # Returns
    /// * `Ok(Some(notification))` - Successfully parsed into an ACP notification
    /// * `Ok(None)` - Valid message but no notification needed (e.g., metadata only)
    /// * `Err(...)` - Parse error or invalid message structure
    pub fn stream_json_to_acp(
        line: &str,
        session_id: &SessionId,
    ) -> Result<Option<SessionNotification>> {
        // Parse the JSON line
        let parsed: JsonValue = serde_json::from_str(line).map_err(|e| {
            let truncated_line: String = line.chars().take(100).collect();
            AgentError::Internal(format!(
                "Malformed JSON: {}. Line: {}...",
                e, truncated_line
            ))
        })?;

        // Check the message type
        let msg_type = parsed.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
            AgentError::Internal("Missing 'type' field in stream-json".to_string())
        })?;

        match msg_type {
            "assistant" => {
                // Parse assistant message
                let assistant_msg: StreamJsonAssistantMessage = serde_json::from_value(parsed)
                    .map_err(|e| {
                        AgentError::Internal(format!("Failed to parse assistant message: {}", e))
                    })?;

                // Validate message type
                assistant_msg.validate()?;

                // Convert first content item to ACP
                if let Some(content_item) = assistant_msg.message.content.first() {
                    // Log if there are additional content items that will be ignored
                    if assistant_msg.message.content.len() > 1 {
                        tracing::debug!(
                            "Assistant message contains {} content items, only returning first",
                            assistant_msg.message.content.len()
                        );
                    }

                    let content_block = Self::parse_content_item(content_item)?;

                    Ok(Some(SessionNotification {
                        session_id: session_id.clone(),
                        update: SessionUpdate::AgentMessageChunk {
                            content: content_block,
                        },
                        meta: None,
                    }))
                } else {
                    Ok(None)
                }
            }
            "system" => {
                // System messages are metadata only, don't notify
                tracing::debug!("Received system message (metadata only)");
                Ok(None)
            }
            "result" => {
                // Result messages are metadata only, don't notify
                tracing::debug!("Received result message (metadata only)");
                Ok(None)
            }
            _ => {
                tracing::warn!("Unknown stream-json message type: {}", msg_type);
                Ok(None)
            }
        }
    }

    /// Convert a content item from stream-json to ACP ContentBlock
    ///
    /// Note: Tool use is converted to text representation because ACP ContentBlock
    /// does not currently have a ToolUse variant. The ACP protocol defines ContentBlock
    /// as: Text, Image, Audio, ResourceLink, and Resource only.
    ///
    /// The text representation uses single-line JSON format to match stream-json conventions.
    fn parse_content_item(item: &ContentItem) -> Result<ContentBlock> {
        match item {
            ContentItem::Text { text, .. } => Ok(ContentBlock::Text(TextContent {
                text: text.clone(),
                annotations: None,
                meta: None,
            })),
            ContentItem::ToolUse {
                id, name, input, ..
            } => {
                // ACP ContentBlock does not have a ToolUse variant, so convert to text
                let tool_json = serde_json::json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input,
                });
                Ok(ContentBlock::Text(TextContent {
                    text: serde_json::to_string(&tool_json).unwrap_or_default(),
                    annotations: None,
                    meta: None,
                }))
            }
        }
    }

    /// Convert tool result to stream-json for claude stdin
    ///
    /// # Arguments
    /// * `tool_call_id` - The ID of the tool call this result is for
    /// * `result` - The result content as a string
    ///
    /// # Returns
    /// A JSON string formatted for stream-json input
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn tool_result_to_stream_json(tool_call_id: &str, result: &str) -> Result<String> {
        let message = StreamJsonToolResultMessage {
            r#type: "user".to_string(),
            message: ToolResultMessage {
                role: "user".to_string(),
                content: vec![ToolResultContent {
                    r#type: "tool_result".to_string(),
                    tool_use_id: tool_call_id.to_string(),
                    content: vec![ToolResultTextContent {
                        r#type: "text".to_string(),
                        text: result.to_string(),
                    }],
                }],
            },
        };

        serde_json::to_string(&message).map_err(|e| {
            AgentError::Internal(format!("Failed to serialize tool result message: {}", e))
        })
    }
}

// Internal wire format types for stream-json

#[derive(Serialize, Deserialize)]
struct StreamJsonUserMessage {
    r#type: String,
    message: UserMessage,
}

#[derive(Serialize, Deserialize)]
struct UserMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct StreamJsonAssistantMessage {
    r#type: String,
    message: AssistantMessage,
}

impl StreamJsonAssistantMessage {
    /// Validate that the message type is correct
    fn validate(&self) -> Result<()> {
        if self.r#type != "assistant" {
            return Err(AgentError::Internal(format!(
                "Expected message type 'assistant', got '{}'",
                self.r#type
            )));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct AssistantMessage {
    content: Vec<ContentItem>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentItem {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: JsonValue,
    },
}

#[derive(Serialize)]
struct StreamJsonToolResultMessage {
    r#type: String,
    message: ToolResultMessage,
}

#[derive(Serialize)]
struct ToolResultMessage {
    role: String,
    content: Vec<ToolResultContent>,
}

#[derive(Serialize)]
struct ToolResultContent {
    r#type: String,
    tool_use_id: String,
    content: Vec<ToolResultTextContent>,
}

#[derive(Serialize)]
struct ToolResultTextContent {
    r#type: String,
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_to_stream_json_simple_text() {
        // Test: Convert simple text message from ACP to stream-json
        let content = vec![ContentBlock::Text(TextContent {
            text: "Hello, world!".to_string(),
            annotations: None,
            meta: None,
        })];

        let result = ProtocolTranslator::acp_to_stream_json(content);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert_eq!(parsed["message"]["content"], "Hello, world!");
    }

    #[test]
    fn test_stream_json_to_acp_assistant_text() {
        // Test: Convert assistant text message from stream-json to ACP
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello back!"}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(notification.is_some());

        let notification = notification.unwrap();
        assert_eq!(notification.session_id, session_id);

        match notification.update {
            SessionUpdate::AgentMessageChunk { content } => {
                if let ContentBlock::Text(text) = content {
                    assert_eq!(text.text, "Hello back!");
                } else {
                    panic!("Expected text content block");
                }
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }

    #[test]
    fn test_stream_json_to_acp_system_message() {
        // Test: System messages should return None (metadata only)
        let line = r#"{"type":"system","subtype":"init","session_id":"test"}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_stream_json_to_acp_result_message() {
        // Test: Result messages should return None (metadata only)
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.114}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_tool_result_to_stream_json() {
        // Test: Convert tool result to stream-json
        let tool_call_id = "toolu_123";
        let result_text = "File contents here";

        let result = ProtocolTranslator::tool_result_to_stream_json(tool_call_id, result_text);
        assert!(result.is_ok());

        let json_str = result.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert!(parsed["message"]["content"].is_array());

        let content = &parsed["message"]["content"][0];
        assert_eq!(content["type"], "tool_result");
        assert_eq!(content["tool_use_id"], tool_call_id);
        assert_eq!(content["content"][0]["type"], "text");
        assert_eq!(content["content"][0]["text"], result_text);
    }

    #[test]
    fn test_stream_json_to_acp_malformed_json() {
        // Test: Malformed JSON should return error
        let line = r#"{"type":"assistant", invalid json"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_json_to_acp_missing_type() {
        // Test: Missing type field should return error
        let line = r#"{"message":{"content":[{"type":"text","text":"Hello"}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_json_to_acp_unknown_type() {
        // Test: Unknown type should return None (skip with warning)
        let line = r#"{"type":"unknown_type","data":"something"}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_stream_json_to_acp_assistant_tool_use() {
        // Test: Convert assistant tool use message from stream-json to ACP
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"read_file","input":{"path":"test.txt"}}]}}"#;
        let session_id = SessionId("test_session".into());

        let result = ProtocolTranslator::stream_json_to_acp(line, &session_id);
        assert!(result.is_ok());

        let notification = result.unwrap();
        assert!(notification.is_some());

        let notification = notification.unwrap();
        match notification.update {
            SessionUpdate::AgentMessageChunk { content } => {
                if let ContentBlock::Text(text) = content {
                    // Tool use is temporarily converted to text
                    assert!(text.text.contains("tool_use"));
                    assert!(text.text.contains("toolu_123"));
                    assert!(text.text.contains("read_file"));
                } else {
                    panic!("Expected text content block");
                }
            }
            _ => panic!("Expected AgentMessageChunk"),
        }
    }
}
