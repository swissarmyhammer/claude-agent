# Implement User Permission Interaction

## Description
Implement actual user interaction for permission requests instead of auto-selecting responses.

## Location
`lib/src/agent.rs:3176`

## Code Context
```rust
// TODO: Implement actual user interaction
// For now, we'll still auto-select "allow-once" but in a real implementation
```

## Current Behavior
Currently auto-selects "allow-once" for all permission requests.

## Implementation Notes
- Design user interaction flow for permission requests
- Support multiple response options (allow-once, allow-always, deny)
- Add timeout handling for user responses
- Consider batch permission requests
- Maintain user preference history


## Proposed Solution

After analyzing the codebase, I've identified the following implementation approach:

### Current State Analysis
- The permission system at `/Users/wballard/github/claude-agent/lib/src/agent.rs:3357` currently auto-selects "allow-once" for all RequireUserConsent policy results
- Permission infrastructure exists with:
  - `PermissionOption` struct with `option_id`, `name`, and `kind` (AllowOnce, AllowAlways, RejectOnce, RejectAlways)
  - `PermissionRequest` and `PermissionResponse` structs for ACP protocol
  - `PermissionPolicyEngine` for evaluating tool calls
  - Permission options are already generated but not presented to users

### Implementation Steps

1. **Create User Prompt Handler**
   - Add a new module `lib/src/user_prompt.rs` with a trait `UserPromptHandler`
   - Implement a console-based handler using `tokio::io::stdin()` for CLI interaction
   - Support formatting permission options as a numbered menu
   - Handle user selection via keyboard input

2. **Add Permission Preference Storage**
   - Create a new module `lib/src/permission_storage.rs`
   - Store "always" decisions (allow-always, reject-always) in memory with tool name as key
   - Use `Arc<RwLock<HashMap<String, PermissionOptionKind>>>` for thread-safe access
   - Check stored preferences before prompting user

3. **Integrate User Interaction in request_permission**
   - In the `RequireUserConsent` branch at line 3357:
     - Check permission storage for existing "always" decisions
     - If found, return the stored decision immediately
     - Otherwise, prompt user with available options via `UserPromptHandler`
     - Store decision if user selected "always" variant
   - Add timeout using `tokio::time::timeout` (default 60 seconds)
   - Return `Cancelled` outcome on timeout

4. **Add UserPromptHandler to ClaudeAgent**
   - Add field `user_prompt_handler: Arc<dyn UserPromptHandler + Send + Sync>` to `ClaudeAgent`
   - Initialize in constructor with default console handler
   - Allow injection for testing

5. **Testing Strategy**
   - Create mock `UserPromptHandler` for unit tests
   - Test all permission option paths (allow-once, allow-always, reject-once, reject-always)
   - Test timeout behavior
   - Test permission storage for "always" decisions
   - Test that "once" decisions don't get stored

### Design Decisions
- Using trait for `UserPromptHandler` allows different implementations (CLI, GUI, test mocks)
- In-memory storage for "always" decisions is sufficient for MVP (can be extended to persistent storage later)
- Timeout of 60 seconds balances user convenience with preventing hung operations
- Console prompts use numbered menu for ease of use


## Implementation Complete

### Changes Made

1. **Created `user_prompt.rs` module** (`/Users/wballard/github/claude-agent/lib/src/user_prompt.rs`)
   - `UserPromptHandler` trait defining the interface for prompting users
   - `ConsolePromptHandler` implementation for CLI interaction using `tokio::io::stdin()`
   - `MockPromptHandler` for testing that returns predetermined responses
   - User-friendly console UI with formatted permission request display

2. **Created `permission_storage.rs` module** (`/Users/wballard/github/claude-agent/lib/src/permission_storage.rs`)
   - `PermissionStorage` struct using `Arc<RwLock<HashMap>>` for thread-safe storage
   - Stores only "always" decisions (AllowAlways, RejectAlways)
   - Ignores "once" decisions as they shouldn't persist
   - Methods: `get_preference`, `store_preference`, `clear_all`, `remove_preference`

3. **Modified `ClaudeAgent` struct** (`/Users/wballard/github/claude-agent/lib/src/agent.rs:368`)
   - Added `user_prompt_handler: Arc<dyn UserPromptHandler>` field
   - Added `permission_storage: Arc<PermissionStorage>` field
   - Default constructor uses `ConsolePromptHandler`
   - Test constructor `new_with_prompt_handler` allows injecting mock handlers

4. **Implemented User Interaction in `request_permission`** (`/Users/wballard/github/claude-agent/lib/src/agent.rs:3361`)
   - Checks permission storage for existing "always" decisions first
   - Returns stored decision immediately if found
   - Prompts user via `UserPromptHandler` with 60-second timeout
   - Handles timeout by returning `Cancelled` outcome
   - Stores user decision if "always" option selected
   - Fixed hardcoded log message to show actual outcome

5. **Added `PartialEq` and `Eq` to `PermissionOptionKind`** (`/Users/wballard/github/claude-agent/lib/src/tools.rs:174`)
   - Required for testing and comparison operations

6. **Created comprehensive tests** (`/Users/wballard/github/claude-agent/lib/src/permission_interaction_tests.rs`)
   - Tests for `MockPromptHandler` functionality
   - Tests for permission storage (store, retrieve, clear, remove)
   - Tests verify "once" decisions are not stored
   - Tests verify "always" decisions are persisted

7. **Updated existing tests** (`/Users/wballard/github/claude-agent/lib/src/agent.rs:3983`)
   - Modified `create_test_agent()` to use `MockPromptHandler` with "allow-once" default
   - All 35 permission-related tests passing

### Key Features Implemented

✅ User prompt mechanism with console interface
✅ Permission preference storage for "always" decisions  
✅ 60-second timeout on user prompts
✅ Automatic use of stored preferences
✅ Mock handler for testing
✅ All tests passing (35/35)

### Code Quality

- Zero compilation warnings
- All tests pass
- Thread-safe implementation using `Arc` and `RwLock`
- Follows Rust best practices
- Clear separation of concerns
- Testable design with trait-based abstraction


## Code Review Improvements Completed

All high and medium priority items from code review have been addressed:

### Documentation Enhancements
1. **Added doc comments to ClaudeAgent fields**
   - Documented `user_prompt_handler` field explaining its purpose for interactive user prompts
   - Documented `permission_storage` field explaining in-memory preference storage

2. **Enhanced PermissionStorage documentation**
   - Updated module-level docs to clarify in-memory storage
   - Added struct-level docs explaining session-level preference isolation
   - Documented why preferences don't persist across restarts (intentional design)

3. **Documented count() method rationale**
   - Explained public API purpose for monitoring and debugging
   - Noted use cases: UI components, external tools

4. **Added module-level doc comment to permission_interaction_tests.rs**
   - Documents complete permission workflow scenarios covered
   - Lists all test categories: mock handler, storage operations, allow/reject flows

### Code Quality Improvements
1. **Made permission prompt timeout a named constant**
   - Created `PERMISSION_PROMPT_TIMEOUT_SECS` constant at module level
   - Changed from hardcoded `Duration::from_secs(60)` to named constant
   - Improves maintainability and makes timeout duration explicit

### Test Coverage Enhancements
1. **Added integration test for allow-always permission flow**
   - `test_allow_always_stores_preference_and_auto_allows_next_call`
   - Tests complete flow: user selection → storage → retrieval for next call
   - Verifies preference is stored and can be retrieved

2. **Added integration test for reject-always permission flow**
   - `test_reject_always_stores_preference_and_auto_rejects_next_call`
   - Tests complete flow: user selection → storage → retrieval for next call
   - Verifies rejection preference is stored and can be retrieved

### Verification
- ✅ All changes compile without warnings
- ✅ All 629 tests pass (3 leaky)
- ✅ CODE_REVIEW.md removed after completion

### Optional Enhancements Not Implemented
The following low-priority items were not implemented as they are optional enhancements:
- Moving console output to stderr (would require more extensive changes)
- Deriving Hash for PermissionOptionKind (not needed for current use cases)
