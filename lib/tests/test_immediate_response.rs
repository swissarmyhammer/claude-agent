//! Test that agent responds immediately to first prompt
//!
//! This test reproduces the bug where agent sends thoughts/plans but no
//! AgentMessageChunk until multiple prompts are sent.

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
async fn test_agent_responds_immediately_to_first_prompt() {
    // Skip if ANTHROPIC_API_KEY is not set
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Skipping test_agent_responds_immediately_to_first_prompt - ANTHROPIC_API_KEY not set");
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

    // Skip authentication - agent declares no auth methods and rejects all auth attempts

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

    // Send first prompt: "Say hello"
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

    // Print what we received
    println!("\n=== Received {} notifications ===", notifications.len());
    for (i, notif) in notifications.iter().enumerate() {
        match &notif.update {
            SessionUpdate::AgentMessageChunk { content } => {
                if let ContentBlock::Text(text) = content {
                    println!("{}. AgentMessageChunk: {}", i + 1, text.text);
                }
            }
            SessionUpdate::AgentThoughtChunk { content } => {
                if let ContentBlock::Text(text) = content {
                    println!("{}. AgentThoughtChunk: {}", i + 1, text.text);
                }
            }
            SessionUpdate::UserMessageChunk { content } => {
                if let ContentBlock::Text(text) = content {
                    println!("{}. UserMessageChunk: {}", i + 1, text.text);
                }
            }
            SessionUpdate::Plan(_) => {
                println!("{}. Plan", i + 1);
            }
            SessionUpdate::AvailableCommandsUpdate { available_commands } => {
                println!("{}. AvailableCommandsUpdate: {} commands", i + 1, available_commands.len());
            }
            _ => {
                println!("{}. {:?}", i + 1, notif.update);
            }
        }
    }

    // Assert that we received at least one AgentMessageChunk
    let has_agent_message = notifications.iter().any(|notif| {
        matches!(notif.update, SessionUpdate::AgentMessageChunk { .. })
    });

    assert!(
        has_agent_message,
        "Expected at least one AgentMessageChunk in response to first prompt, but got none!\n\
         Received {} notifications total. This is the bug where agent only sends thoughts/plans but no actual response.",
        notifications.len()
    );

    // Also verify we got some kind of response (not just silence)
    assert!(
        !notifications.is_empty(),
        "Expected some notifications from agent, but got none"
    );
}
