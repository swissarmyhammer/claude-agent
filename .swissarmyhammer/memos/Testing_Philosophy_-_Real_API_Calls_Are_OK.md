# Testing Philosophy: Real API Calls Are OK

## Decision
Tests that call the real Claude API and cost money are acceptable and should NOT be marked with `#[ignore]` or mocked away.

## Rationale
- The marginal cost of a few API calls during testing is negligible compared to developer time
- Real API tests verify actual integration, not just mocked behavior
- Running these tests gives confidence that the SDK integration actually works
- Developer time (including AI assistant time) costs more than a few cents of API calls

## Implementation
- Tests that make real API calls are marked with clear comments: `// NOTE: This test makes a real API call to Claude and costs money. This is intentional.`
- No mocking infrastructure is required for the Claude SDK wrapper
- Tests run normally in CI/CD if API keys are available

## Tests Affected
- `lib/src/claude.rs::test_basic_query` - Tests simple query without context
- `lib/src/claude.rs::test_query_with_context` - Tests query with conversation history

## Date
2025-10-08