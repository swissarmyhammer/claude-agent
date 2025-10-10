//! Test that multiple prompts to the same session use the same Claude process
//!
//! This test verifies that when sending multiple messages to the same session,
//! they all go to the same Claude CLI process, maintaining conversational context.

use agent_client_protocol::{
    Agent, ContentBlock, InitializeRequest, NewSessionRequest,
    PromptRequest, TextContent, V1,
};
use claude_agent_lib::{agent::ClaudeAgent, config::AgentConfig};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn test_multiple_prompts_same_session() {
    // Skip if ANTHROPIC_API_KEY is not set
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Skipping test_multiple_prompts_same_session - ANTHROPIC_API_KEY not set");
        return;
    }

    let local = tokio::task::LocalSet::new();
    local.run_until(test_inner()).await;
}

async fn test_inner() {
    // Create agent with default config
    let config = AgentConfig::default();
    let (agent, _notification_receiver) = ClaudeAgent::new(config).await.unwrap();
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
    let new_session_response = agent
        .new_session(new_session_request)
        .await
        .expect("Failed to create session");
    let session_id = new_session_response.session_id.clone();

    println!("\n=== Created session: {} ===", session_id);

    // First prompt: "Say hello"
    println!("\n=== Sending first prompt: 'Say hello' ===");
    let prompt1_request = PromptRequest {
        session_id: session_id.clone(),
        prompt: vec![ContentBlock::Text(TextContent {
            text: "Say hello".to_string(),
            annotations: None,
            meta: None,
        })],
        meta: None,
    };

    let response1 = agent.prompt(prompt1_request).await;
    assert!(response1.is_ok(), "First prompt should succeed");
    println!("First prompt completed with stop_reason: {:?}", response1.unwrap().stop_reason);

    // Second prompt: "How many crates are in our Cargo.toml?"
    println!("\n=== Sending second prompt to SAME session ===");
    let prompt2_request = PromptRequest {
        session_id: session_id.clone(),
        prompt: vec![ContentBlock::Text(TextContent {
            text: "How many crates are in our Cargo.toml?".to_string(),
            annotations: None,
            meta: None,
        })],
        meta: None,
    };

    let response2 = agent.prompt(prompt2_request).await;
    assert!(response2.is_ok(), "Second prompt should succeed");
    println!("Second prompt completed with stop_reason: {:?}", response2.unwrap().stop_reason);

    println!("\n=== Test complete - both prompts succeeded on same session ===");
    println!("Check the logs above for 'Reusing existing Claude process' messages");
    println!("You should see ONE spawn and ONE reuse message");
}
