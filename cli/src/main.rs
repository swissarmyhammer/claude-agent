//! Claude Agent CLI
//!
//! A command-line interface for starting the Claude Agent ACP server.

use anyhow::Result;
use clap::Parser;
use claude_agent_lib::{AgentConfig, ClaudeAgentServer};

/// Claude Agent CLI - Agent Client Protocol server for Claude Code
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(&cli.log_level)
        .init();

    // Create configuration
    let mut config = AgentConfig::default();
    config.server.log_level = cli.log_level.clone();

    // Create and start server
    let server = ClaudeAgentServer::new(config)?;

    server.start_stdio().await?;

    Ok(())
}
