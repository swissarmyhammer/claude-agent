# Replace Sequential Request Handling Placeholder

## Description
The server implementation has placeholder logic for request handling that needs proper implementation.

## Found Issues
- `server.rs:1`: Sequential request handling is a placeholder implementation
- Proper request processing pipeline needed

## Priority
High - Core server functionality

## Files Affected
- `lib/src/server.rs`


## Analysis

The current implementation in `start_with_streams` (lines 117-153) has a placeholder using `tokio::select!` that only runs ONE of two branches:
1. Request handling via `handle_requests`
2. Notification broadcasting via notification receiver

The issue is that `tokio::select!` completes when the FIRST branch completes, so either:
- Requests are handled but notifications are never sent, OR
- Notifications are sent but requests are never processed

## Proposed Solution

Replace the `tokio::select!` with `tokio::join!` or spawn concurrent tasks to handle both:

1. **Spawn request handler task**: Process incoming JSON-RPC requests from the reader
2. **Spawn notification handler task**: Forward session notifications to the writer
3. **Wait for both**: Use `tokio::join!` or join handles to ensure both run concurrently
4. **Error handling**: Propagate errors from either task appropriately

This ensures:
- Requests are continuously processed
- Notifications are continuously forwarded
- Both operations happen concurrently as intended for a production system

### Implementation Steps

1. Replace `tokio::select!` with spawned tasks for concurrent execution
2. Create task handles for both request and notification handling
3. Use `tokio::join!` to wait for both tasks
4. Handle errors from either task appropriately
5. Ensure proper cleanup on task completion or failure




## Implementation Progress

### Current Status
Implemented concurrent request and notification handling using `tokio::join!` with a shutdown coordination channel. However, the test is failing because the duplex stream setup isn't working correctly.

### Root Cause Found
The test creates two separate duplex channels:
1. `(client_writer, server_reader)` - for client→server communication  
2. `(server_writer, client_reader)` - for server→client communication

This should work in theory, but data isn't flowing. The server starts reading from `server_reader` but never receives data written to `client_writer`.

### Next Steps
1. Fix the test stream setup to ensure bidirectional communication works
2. Verify single request/response works first
3. Then verify concurrent behavior with both requests and notifications




## Final Implementation

### Solution
Replaced the `tokio::select!` placeholder with concurrent request and notification handling using `tokio::join!` with shutdown coordination:

1. **Concurrent Handlers**: Both request handler and notification handler run concurrently via `tokio::join!`
2. **Shutdown Coordination**: A broadcast channel coordinates shutdown - when request handler completes (connection closed), it signals the notification handler to stop
3. **Clean Termination**: Both handlers complete gracefully, notification handler stops when receiving shutdown signal

### Key Changes in `start_with_streams` (lib/src/server.rs:116-178)
- Created shutdown broadcast channel for coordination
- Request handler signals shutdown when complete
- Notification handler listens for both notifications and shutdown signal using nested `tokio::select!`
- Both handlers run concurrently via `tokio::join!`

### Testing
- Added comprehensive test `test_concurrent_request_and_notification_handling` that verifies both request processing and concurrent operation
- Test spawns client task independently to ensure true concurrency
- All 632 tests pass

### Result
The server now properly handles requests and notifications concurrently as intended for production use, eliminating the placeholder sequential handling.




## Code Review Improvements

Applied code review suggestions to improve documentation and maintainability:

1. **Enhanced Documentation** (lines 116-134)
   - Added comprehensive doc comments for `start_with_streams` method
   - Documented concurrency model using `tokio::join!`
   - Explained shutdown flow step by step
   - Clarified rationale for using broadcast channel vs oneshot

2. **Improved Error Logging** (line 169)
   - Enhanced notification handler error message to include shutdown context
   - Changed from "Failed to send notification: {}" to "Failed to send notification: {} - shutting down notification handler"

3. **Documented Magic Numbers** (line 147)
   - Added comment explaining broadcast channel capacity of 1
   - Clarifies that single shutdown signal is sufficient

### Verification
- ✅ All 632 tests pass (2 leaky)
- ✅ No clippy warnings
- ✅ Cargo build successful
- ✅ Code maintains production quality

### Remaining Suggestions (Optional)
The code review identified two additional optional improvements that are not critical:
- Test naming: Could split the concurrent test into smaller focused tests
- Test sleep: Could replace 50ms sleep with sync mechanism (current approach is pragmatic)