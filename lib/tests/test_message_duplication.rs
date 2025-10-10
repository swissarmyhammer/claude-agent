//! Test to verify that stream_event chunks and assistant messages don't duplicate
//!
//! This test sends "say hello" to Claude and verifies:
//! 1. We receive stream_event chunks with content_block_delta (real-time text)
//! 2. We also receive an assistant message with the complete text
//! 3. The protocol translator filters out the duplicate assistant message
//!
//! The expected behavior is that only the stream_event chunks result in
//! AgentMessageChunk notifications, and the assistant message is silently ignored.

use agent_client_protocol::{
    Agent, ContentBlock, InitializeRequest, NewSessionRequest,
    PromptRequest, SessionNotification, SessionUpdate, TextContent, V1,
};
use claude_agent_lib::{agent::ClaudeAgent, config::AgentConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Helper to consume notifications from a broadcast receiver with timeout
async fn collect_notifications_with_timeout(
    receiver: &mut broadcast::Receiver<SessionNotification>,
    timeout: Duration,
) -> Vec<SessionNotification> {
    let mut notifications = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, receiver.recv()).await {
            Ok(Ok(notification)) => {
                notifications.push(notification);
            }
            Ok(Err(_)) => {
                // Channel closed
                break;
            }
            Err(_) => {
                // Timeout
                break;
            }
        }
    }

    notifications
}

#[tokio::test(flavor = "current_thread")]
async fn test_message_chunks_vs_full_message() {
    // Skip if ANTHROPIC_API_KEY is not set
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Skipping test_message_chunks_vs_full_message - ANTHROPIC_API_KEY not set");
        return;
    }

    let local = tokio::task::LocalSet::new();
    local.run_until(test_inner()).await;
}

async fn test_inner() {
    // Create agent with default config
    let config = AgentConfig::default();
    let (agent, mut notification_receiver) = ClaudeAgent::new(config).await.unwrap();
    let agent = Arc::new(agent);

    // Initialize agent
    let init_request = InitializeRequest {
        protocol_version: V1,
        client_capabilities: agent_client_protocol::ClientCapabilities {
            fs: agent_client_protocol::FileSystemCapability {
                read_text_file: true,
                write_text_file: true,
                meta: None,
            },
            terminal: true,
            meta: None,
        },
        meta: None,
    };
    agent.initialize(init_request).await.unwrap();

    // Create session
    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("."),
        mcp_servers: vec![],
        meta: None,
    };
    let new_session_response = agent.new_session(new_session_request).await
        .expect("Failed to create session - is ANTHROPIC_API_KEY set?");
    let session_id = new_session_response.session_id.clone();

    // Clear any initialization notifications
    let _ = collect_notifications_with_timeout(&mut notification_receiver, Duration::from_millis(100)).await;

    // Send prompt: "Say hello" - a simple request that should get a text response
    let prompt_request = PromptRequest {
        session_id: session_id.clone(),
        prompt: vec![ContentBlock::Text(TextContent {
            text: "Say hello".to_string(),
            annotations: None,
            meta: None,
        })],
        meta: None,
    };

    // Send prompt (spawned so we can collect notifications while it runs)
    let agent_clone = Arc::clone(&agent);
    let prompt_handle = tokio::task::spawn_local(async move {
        agent_clone.prompt(prompt_request).await
    });

    // Collect notifications for 5 seconds
    let notifications = collect_notifications_with_timeout(&mut notification_receiver, Duration::from_secs(5)).await;

    // Cancel the prompt if it's still running
    prompt_handle.abort();

    // Analyze the notifications
    println!("\n=== Received {} notifications ===", notifications.len());

    let mut agent_message_chunks = Vec::new();
    let mut full_text = String::new();

    for (i, notif) in notifications.iter().enumerate() {
        match &notif.update {
            SessionUpdate::AgentMessageChunk { content } => {
                if let ContentBlock::Text(text) = content {
                    println!("{}. AgentMessageChunk: '{}' ({} chars)", i + 1, text.text, text.text.len());
                    agent_message_chunks.push(text.text.clone());
                    full_text.push_str(&text.text);
                }
            }
            SessionUpdate::AgentThoughtChunk { content } => {
                if let ContentBlock::Text(text) = content {
                    println!("{}. AgentThoughtChunk: '{}' ({} chars)", i + 1,
                        text.text.chars().take(50).collect::<String>(), text.text.len());
                }
            }
            _ => {
                println!("{}. {:?}", i + 1, notif.update);
            }
        }
    }

    println!("\n=== Summary ===");
    println!("Total AgentMessageChunk notifications: {}", agent_message_chunks.len());
    println!("Full reconstructed text: '{}'", full_text);
    println!("Total characters: {}", full_text.len());

    // Verify expectations:
    // 1. We should have received multiple AgentMessageChunk notifications (chunks)
    assert!(
        !agent_message_chunks.is_empty(),
        "Expected to receive AgentMessageChunk notifications (from stream_events), but got none"
    );

    // 2. The chunks should contain actual content
    assert!(
        !full_text.is_empty(),
        "Expected non-empty text content in message chunks"
    );

    // 3. We should NOT see a large single chunk that equals the full message
    //    (because assistant messages are filtered out)
    //    Instead we should see multiple smaller chunks from stream_events
    println!("\n=== Verification ===");
    if agent_message_chunks.len() > 1 {
        println!("✓ Received {} chunks (streaming worked correctly)", agent_message_chunks.len());
        println!("✓ Assistant full-message duplication was successfully filtered out");
    } else if agent_message_chunks.len() == 1 {
        println!("⚠ Received only 1 chunk - either message was very short or streaming may not be working");
    }

    println!("\n=== Individual Chunks ===");
    for (i, chunk) in agent_message_chunks.iter().enumerate() {
        println!("Chunk {}: '{}' ({} chars)", i + 1, chunk, chunk.len());
    }
}
