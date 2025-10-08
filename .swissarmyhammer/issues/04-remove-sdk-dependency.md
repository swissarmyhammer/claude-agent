# Phase 4: Remove claude-sdk-rs Dependency

## Goal
Remove the claude-sdk-rs dependency entirely now that we have direct CLI integration.

## Scope
- Remove from Cargo.toml
- Remove SDK imports from codebase
- Clean up error types
- Update documentation
- Verify all tests pass

## Implementation

### Update Cargo.toml
```toml
# REMOVE:
# claude-sdk-rs = "1.0.1"

# Keep everything else
```

### Remove Imports
Files to update:
- `lib/src/claude.rs` - remove `use claude_sdk_rs::*`
- `lib/src/error.rs` - remove SDK error variant
- `lib/src/agent.rs` - verify no SDK imports

### Update Error Types (lib/src/error.rs)
```rust
// REMOVE:
// Claude(claude_sdk_rs::Error),

// ADD (if not already present):
ProcessManagement(String),
ProtocolTranslation(String),
```

### Update Tests
- Remove tests that imported SDK types
- Update mocks if any used SDK types
- Verify all existing tests still pass

### Documentation Updates
- Update README.md to reflect direct CLI usage
- Update architecture docs
- Add note about removed SDK dependency

## Files to Check

Search for `claude_sdk_rs` or `claude-sdk-rs`:
```bash
rg "claude_sdk_rs|claude-sdk-rs" --type rust
rg "claude_sdk_rs|claude-sdk-rs" --type toml
```

## Testing
- Run full test suite: `cargo test`
- Run integration tests: `cargo test --test integration_tests`
- Verify build succeeds: `cargo build --release`
- Check for unused dependencies: `cargo udeps`

## Acceptance Criteria
- [ ] claude-sdk-rs removed from Cargo.toml
- [ ] No SDK imports remain in codebase
- [ ] All tests pass
- [ ] Release build succeeds
- [ ] No unused dependencies
- [ ] Documentation updated

## Dependencies
- Depends on: Phase 3 (integration complete)

## Benefits
- One less dependency to maintain
- Smaller binary size
- Faster compile times
- Direct control over claude CLI
- Clearer architecture
