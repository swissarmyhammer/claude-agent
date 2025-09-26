//! Claude Agent CLI
//!
//! A command-line interface for starting the Claude Agent ACP server.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use claude_agent_lib::config::StreamFormat;
use claude_agent_lib::{AgentConfig, ClaudeAgentServer};
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::{filter::EnvFilter, fmt::format::FmtSpan};

// Standard Unix exit codes
const EX_GENERAL: i32 = 1;
const EX_NOINPUT: i32 = 66;
const EX_NOPERM: i32 = 77;
const EX_CONFIG: i32 = 78;

/// Claude Agent CLI - Agent Client Protocol server for Claude Code
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(name = "claude-agent")]
struct Cli {
    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Enable JSON logging format
    #[arg(long)]
    json_logs: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the ACP server
    Serve {
        /// Port to bind the server (optional, uses stdio by default)
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable development mode (more verbose logging, debug features)
        #[arg(long)]
        dev: bool,
    },
    /// Validate configuration file
    Config {
        /// Configuration file to validate
        config: PathBuf,
    },
    /// Show server information
    Info,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        handle_error(e);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level, cli.json_logs)?;

    info!("Starting Claude Agent CLI v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = load_configuration(cli.config.as_ref()).await?;

    match cli.command.unwrap_or(Commands::Serve {
        port: None,
        dev: false,
    }) {
        Commands::Serve { port, dev } => {
            run_server(config, port, dev).await?;
        }
        Commands::Config { config } => {
            validate_config_file(config).await?;
        }
        Commands::Info => {
            show_info().await?;
        }
    }

    Ok(())
}

/// Initialize the tracing subscriber with the specified log level and format.
///
/// # Arguments
/// * `log_level` - The log level string (trace, debug, info, warn, error)
/// * `json_format` - Whether to use JSON format for logging output
///
/// # Returns
/// * `Result<()>` - Success or error during initialization
fn init_logging(log_level: &str, json_format: bool) -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_span_events(FmtSpan::CLOSE);

    if json_format {
        subscriber.json().init();
    } else {
        subscriber.init();
    }

    info!("Logging initialized with level: {}", log_level);
    Ok(())
}

/// Load agent configuration from file or use defaults.
///
/// Supports JSON, TOML, and YAML configuration file formats.
/// If no path is provided, returns default configuration.
///
/// # Arguments
/// * `config_path` - Optional path to configuration file
///
/// # Returns
/// * `Result<AgentConfig>` - Loaded and validated configuration or error
async fn load_configuration(config_path: Option<&PathBuf>) -> Result<AgentConfig> {
    match config_path {
        Some(path) => {
            info!("Loading configuration from: {}", path.display());

            if !path.exists() {
                return Err(anyhow::anyhow!(
                    "Configuration file not found: {}",
                    path.display()
                ));
            }

            let content = tokio::fs::read_to_string(path)
                .await
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;

            let config: AgentConfig = match path.extension().and_then(|ext| ext.to_str()) {
                Some("json") => serde_json::from_str(&content)
                    .with_context(|| "Failed to parse JSON configuration")?,
                Some("toml") => toml::from_str(&content)
                    .with_context(|| "Failed to parse TOML configuration")?,
                Some("yaml") | Some("yml") => serde_yaml::from_str(&content)
                    .with_context(|| "Failed to parse YAML configuration")?,
                _ => {
                    return Err(anyhow::anyhow!(
                        "Unsupported configuration file format. Use .json, .toml, or .yaml"
                    ));
                }
            };

            config
                .validate()
                .with_context(|| "Configuration validation failed")?;

            info!("Configuration loaded successfully");
            Ok(config)
        }
        None => {
            info!("Using default configuration");
            Ok(AgentConfig::default())
        }
    }
}

/// Display comprehensive information about the Claude Agent CLI.
///
/// Shows version information, supported features, configuration defaults,
/// and usage examples to help users understand available capabilities.
///
/// # Returns
/// * `Result<()>` - Success or error during information display
async fn show_info() -> Result<()> {
    println!("Claude Agent Information");
    println!("========================");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Authors: {}", env!("CARGO_PKG_AUTHORS"));
    println!("Description: {}", env!("CARGO_PKG_DESCRIPTION"));
    println!();

    println!("Supported Features:");
    println!("  ✓ Agent Client Protocol (ACP) v1.0.0");
    println!("  ✓ Streaming responses");
    println!("  ✓ File system operations (read, write, list)");
    println!("  ✓ Terminal operations");
    println!("  ✓ Session management");
    println!("  ✓ Tool call permissions");
    println!();

    println!("Configuration:");
    let default_config = AgentConfig::default();
    println!("  Default model: {}", default_config.claude.model);
    println!(
        "  Default security patterns: {} allowed",
        default_config.security.allowed_file_patterns.len()
    );
    println!(
        "  Default MCP servers: {}",
        default_config.mcp_servers.len()
    );
    println!();

    println!("Usage:");
    println!("  claude-agent serve           # Start server on stdio");
    println!("  claude-agent serve -p 3000   # Start server on port 3000");
    println!("  claude-agent config FILE     # Validate configuration");
    println!("  claude-agent info            # Show this information");

    Ok(())
}

/// Handle fatal errors by logging and exiting with appropriate exit codes.
///
/// Maps different error types to standard Unix exit codes:
/// * 78 (EX_CONFIG) - Configuration errors
/// * 77 (EX_NOPERM) - Permission errors
/// * 66 (EX_NOINPUT) - File not found errors
/// * 1 - General errors
///
/// # Arguments
/// * `error` - The error that caused the fatal condition
fn handle_error(error: anyhow::Error) -> ! {
    error!("Fatal error: {}", error);

    // Print error chain for better debugging
    let mut current = error.source();
    while let Some(cause) = current {
        error!("  Caused by: {}", cause);
        current = cause.source();
    }

    // Determine exit code based on error type and context
    let exit_code = classify_error(&error);

    std::process::exit(exit_code);
}

/// Classify error types to determine appropriate exit codes.
fn classify_error(error: &anyhow::Error) -> i32 {
    let error_msg = error.to_string().to_lowercase();

    if error_msg.contains("configuration") || error_msg.contains("config") {
        EX_CONFIG
    } else if error_msg.contains("permission")
        || error_msg.contains("access")
        || error_msg.contains("denied")
    {
        EX_NOPERM
    } else if error_msg.contains("not found") || error_msg.contains("no such file") {
        EX_NOINPUT
    } else {
        // Check the error chain for more specific error types
        let mut current = error.source();
        while let Some(cause) = current {
            let cause_msg = cause.to_string().to_lowercase();
            if cause_msg.contains("permission") || cause_msg.contains("access") {
                return EX_NOPERM;
            } else if cause_msg.contains("not found") {
                return EX_NOINPUT;
            }
            current = cause.source();
        }

        EX_GENERAL
    }
}

/// Start the Claude Agent ACP server with the given configuration.
///
/// Initializes and runs the Agent Client Protocol server, handling both
/// stdio and TCP port modes. Supports development mode for enhanced debugging.
///
/// # Arguments
/// * `config` - Agent configuration to use for the server
/// * `port` - Optional TCP port to bind (uses stdio if None)
/// * `dev` - Enable development mode features
///
/// # Returns
/// * `Result<()>` - Success or error during server operation
async fn run_server(mut config: AgentConfig, port: Option<u16>, dev: bool) -> Result<()> {
    if dev {
        info!("Development mode enabled");
        info!("  - Enhanced error logging enabled");
        info!("  - Standard stream format for debugging");
        info!("  - Detailed server configuration logging enabled");

        // In dev mode, use Standard format for better readability during development
        config.claude.stream_format = StreamFormat::Standard;

        // Log detailed configuration in dev mode
        info!("Development mode configuration:");
        info!("  - Claude model: {}", config.claude.model);
        info!("  - Stream format: {:?}", config.claude.stream_format);
        info!("  - Server port: {:?}", config.server.port);
        info!(
            "  - Security patterns: {} allowed, {} forbidden",
            config.security.allowed_file_patterns.len(),
            config.security.forbidden_paths.len()
        );
    }

    // Override port from CLI if provided
    if let Some(p) = port {
        config.server.port = Some(p);
        info!("Port overridden from CLI: {}", p);
    }

    info!("Creating ACP server with configuration:");
    info!("  - Model: {}", config.claude.model);
    info!("  - Streaming: enabled");
    info!("  - Port: {:?}", config.server.port);

    let server = ClaudeAgentServer::new(config)
        .await
        .with_context(|| "Failed to create Claude Agent server")?;

    // Use the server's built-in shutdown handling if available, otherwise stdio
    match server.start_with_shutdown().await {
        Ok(()) => {
            info!("Server shut down gracefully");
            Ok(())
        }
        Err(e) => {
            error!("Server error: {}", e);
            Err(e.into())
        }
    }
}

/// Validate a configuration file and display its summary.
///
/// Loads and validates the specified configuration file, then displays
/// a summary of its contents including model, security settings, and MCP servers.
///
/// # Arguments
/// * `config_path` - Path to the configuration file to validate
///
/// # Returns
/// * `Result<()>` - Success if valid, error with details if invalid
async fn validate_config_file(config_path: PathBuf) -> Result<()> {
    info!("Validating configuration file: {}", config_path.display());

    match load_configuration(Some(&config_path)).await {
        Ok(config) => {
            info!("✓ Configuration file is valid");
            info!("Configuration summary:");
            info!("  - Claude model: {}", config.claude.model);
            info!("  - Streaming format: {:?}", config.claude.stream_format);
            info!(
                "  - Security patterns: {} allowed, {} forbidden",
                config.security.allowed_file_patterns.len(),
                config.security.forbidden_paths.len()
            );
            info!("  - MCP servers: {}", config.mcp_servers.len());
            Ok(())
        }
        Err(e) => {
            error!("✗ Configuration file is invalid: {}", e);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use tempfile::TempDir;

    #[test]
    fn test_cli_parsing() {
        let cmd = Cli::command();

        // Test default command
        let matches = cmd.clone().try_get_matches_from(vec!["claude-agent"]);
        assert!(matches.is_ok());

        // Test serve command with port
        let matches = cmd
            .clone()
            .try_get_matches_from(vec!["claude-agent", "serve", "-p", "3000"]);
        assert!(matches.is_ok());

        // Test config command
        let matches = cmd
            .clone()
            .try_get_matches_from(vec!["claude-agent", "config", "test.json"]);
        assert!(matches.is_ok());

        // Test info command
        let matches = cmd
            .clone()
            .try_get_matches_from(vec!["claude-agent", "info"]);
        assert!(matches.is_ok());
    }

    #[test]
    fn test_info_command_parsing() {
        let cli = Cli::try_parse_from(vec!["claude-agent", "info"]).unwrap();

        match cli.command.unwrap() {
            Commands::Info => {
                // Expected
            }
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_serve_command_parsing() {
        let cli =
            Cli::try_parse_from(vec!["claude-agent", "serve", "--port", "8080", "--dev"]).unwrap();

        match cli.command.unwrap() {
            Commands::Serve { port, dev } => {
                assert_eq!(port, Some(8080));
                assert!(dev);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn test_config_command_parsing() {
        let cli =
            Cli::try_parse_from(vec!["claude-agent", "config", "/path/to/config.json"]).unwrap();

        match cli.command.unwrap() {
            Commands::Config { config } => {
                assert_eq!(config, PathBuf::from("/path/to/config.json"));
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[tokio::test]
    async fn test_config_loading_default() {
        let config = load_configuration(None).await.unwrap();
        assert_eq!(config.claude.model, "claude-sonnet-4-20250514");
    }

    #[tokio::test]
    async fn test_config_loading_json() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.json");

        let test_config = AgentConfig::default();
        let config_json = serde_json::to_string_pretty(&test_config).unwrap();

        tokio::fs::write(&config_path, config_json).await.unwrap();

        let loaded_config = load_configuration(Some(&config_path)).await.unwrap();

        assert_eq!(loaded_config.claude.model, test_config.claude.model);
    }

    #[tokio::test]
    async fn test_config_validation_valid() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("valid_config.json");

        let test_config = AgentConfig::default();
        let config_json = serde_json::to_string_pretty(&test_config).unwrap();

        tokio::fs::write(&config_path, config_json).await.unwrap();

        let result = validate_config_file(config_path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_validation_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.json");

        let result = validate_config_file(config_path).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_init_logging_with_info_level() {
        let result = init_logging("info", false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_logging_with_json_format() {
        let result = init_logging("debug", true);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_show_info_output() {
        let result = show_info().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_classify_error_config() {
        let error = anyhow::anyhow!("configuration error occurred");
        let exit_code = classify_error(&error);
        assert_eq!(exit_code, EX_CONFIG);
    }

    #[test]
    fn test_classify_error_permission() {
        let error = anyhow::anyhow!("permission denied");
        let exit_code = classify_error(&error);
        assert_eq!(exit_code, EX_NOPERM);
    }

    #[test]
    fn test_classify_error_not_found() {
        let error = anyhow::anyhow!("file not found");
        let exit_code = classify_error(&error);
        assert_eq!(exit_code, EX_NOINPUT);
    }

    #[test]
    fn test_classify_error_general() {
        let error = anyhow::anyhow!("some general error");
        let exit_code = classify_error(&error);
        assert_eq!(exit_code, EX_GENERAL);
    }

    #[tokio::test]
    async fn test_config_loading_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let test_config = AgentConfig::default();
        let config_toml = toml::to_string_pretty(&test_config).unwrap();

        tokio::fs::write(&config_path, config_toml).await.unwrap();

        let loaded_config = load_configuration(Some(&config_path)).await.unwrap();

        assert_eq!(loaded_config.claude.model, test_config.claude.model);
    }

    #[tokio::test]
    async fn test_config_loading_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.yaml");

        let test_config = AgentConfig::default();
        let config_yaml = serde_yaml::to_string(&test_config).unwrap();

        tokio::fs::write(&config_path, config_yaml).await.unwrap();

        let loaded_config = load_configuration(Some(&config_path)).await.unwrap();

        assert_eq!(loaded_config.claude.model, test_config.claude.model);
    }

    #[tokio::test]
    async fn test_config_loading_invalid_format() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.txt");

        tokio::fs::write(&config_path, "invalid config")
            .await
            .unwrap();

        let result = load_configuration(Some(&config_path)).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported configuration file format"));
    }

    #[tokio::test]
    async fn test_config_loading_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid.json");

        tokio::fs::write(&config_path, "{invalid json}")
            .await
            .unwrap();

        let result = load_configuration(Some(&config_path)).await;
        assert!(result.is_err());
    }
}
