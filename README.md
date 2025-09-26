# Claude Agent

A Rust library and CLI tool that implements an Agent Client Protocol (ACP) server, wrapping Claude Code functionality to enable any ACP-compatible client (like Zed, Emacs, Neovim) to interact with Claude Code.

## Overview

Claude Agent provides a bridge between the Agent Client Protocol and Claude Code, allowing developers to use their favorite ACP-compatible editors to interact with Claude's AI capabilities through a standardized protocol.

## Architecture

- **Library (`claude-agent-lib`)**: Core ACP server implementation
- **CLI (`claude-agent-cli`)**: Simple command-line interface to start the server
- **Integration Layer**: Bridge between ACP and Claude Code via claude-sdk-rs

## Building

```bash
cargo build
```

## Running

```bash
cargo run --bin claude-agent
```

## Testing

```bash
cargo test
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.