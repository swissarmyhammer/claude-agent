# Project Structure Setup

Refer to plan.md

## Goal
Set up the basic project structure as a Rust workspace with library and CLI packages.

## Tasks

### 1. Create Cargo Workspace
- Create root `Cargo.toml` defining workspace with `lib` and `cli` members
- Configure workspace-level dependencies and settings

### 2. Library Package Setup  
- Create `lib/Cargo.toml` with core dependencies:
  - `agent-client-protocol = "0.4.3"`
  - `claude-sdk-rs = { version = "1.0.1", features = ["full"] }`
  - `tokio = { version = "1.40", features = ["full"] }`
  - `tracing = "0.1"`
  - `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`
  - `serde = { version = "1.0", features = ["derive"] }`
  - `serde_json = "1.0"`
  - `anyhow = "1.0"`
  - `thiserror = "1.0"`
  - `uuid = { version = "1.10", features = ["v4", "serde"] }`
  - `async-trait = "0.1"`

### 3. CLI Package Setup
- Create `cli/Cargo.toml` with CLI dependencies:
  - `claude-agent-lib = { path = "../lib" }`
  - `clap = { version = "4.5", features = ["derive"] }`
  - `tracing = "0.1"`
  - `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`
  - `tokio = { version = "1.40", features = ["full"] }`
  - `anyhow = "1.0"`

### 4. Directory Structure
Create the following directory structure:
```
claude-agent/
├── Cargo.toml (workspace)
├── README.md  
├── LICENSE
├── .gitignore
├── lib/
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs (placeholder)
│   └── tests/
└── cli/
    ├── Cargo.toml
    ├── src/
    │   └── main.rs (placeholder)
    └── tests/
```

### 5. Basic Files
- Create `.gitignore` with Rust-specific ignores
- Create `LICENSE` file
- Create basic `README.md` with project description
- Add placeholder `lib.rs` and `main.rs` files that compile

## Acceptance Criteria
- `cargo build` succeeds for both packages
- `cargo test` runs (even with no tests yet)
- Directory structure matches specification
- All dependencies resolve correctly