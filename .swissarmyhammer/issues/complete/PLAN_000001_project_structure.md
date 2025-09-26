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

## Proposed Solution

Based on the plan.md file, I will implement the basic Rust workspace structure with library and CLI packages. The implementation will follow this approach:

1. **Create Workspace Root**: Set up a root `Cargo.toml` that defines a workspace with `lib` and `cli` members
2. **Library Package**: Create `lib/` directory with its own `Cargo.toml` containing the core dependencies for ACP server implementation
3. **CLI Package**: Create `cli/` directory with its own `Cargo.toml` that depends on the library and adds CLI-specific dependencies
4. **Project Files**: Create essential project files (.gitignore, LICENSE, README.md)
5. **Placeholder Code**: Add minimal `lib.rs` and `main.rs` files that compile successfully
6. **Validation**: Ensure `cargo build` and `cargo test` work for the complete workspace

The workspace will be designed to support the Claude Agent ACP Implementation as described in the plan, with the library containing the core ACP server implementation and the CLI providing a simple command-line interface to start the server.

## Implementation Notes

Successfully implemented the basic Rust workspace structure as specified. Here's what was accomplished:

### ✅ Completed Tasks

1. **Workspace Configuration**: Created root `Cargo.toml` with workspace members "lib" and "cli", including shared workspace dependencies.

2. **Library Package**: Created `lib/` directory structure with:
   - `Cargo.toml` with core dependencies for ACP server implementation
   - `src/lib.rs` with placeholder types and basic server structure
   - Basic test coverage (2 passing tests)

3. **CLI Package**: Created `cli/` directory structure with:
   - `Cargo.toml` with CLI-specific dependencies
   - `src/main.rs` with clap-based argument parsing and server initialization
   
4. **Project Files**: 
   - Updated `.gitignore` with Rust-specific ignores (preserved existing SwissArmyHammer entries)
   - Created MIT `LICENSE` file
   - Created basic `README.md` with project overview

5. **Build Validation**: 
   - ✅ `cargo build` succeeds for entire workspace
   - ✅ `cargo test` runs successfully (2 tests passing)
   - All dependencies resolve correctly

### Directory Structure
```
claude-agent/
├── Cargo.toml (workspace root)
├── README.md
├── LICENSE
├── .gitignore
├── lib/
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs (placeholder with basic types)
│   └── tests/
└── cli/
    ├── Cargo.toml
    ├── src/
    │   └── main.rs (placeholder with CLI interface)
    └── tests/
```

### Key Dependencies Configured
- **Core**: `agent-client-protocol` (0.4.3), `claude-sdk-rs` (1.0.1), `tokio`, `tracing`
- **CLI**: `clap` for argument parsing
- **Error Handling**: `anyhow`, `thiserror`  
- **Serialization**: `serde`, `serde_json`

The project structure is now ready for implementing the full ACP server as outlined in the plan.