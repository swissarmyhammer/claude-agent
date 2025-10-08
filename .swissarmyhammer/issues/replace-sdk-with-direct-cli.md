# Replace claude-sdk-rs with ACP Proxy for Persistent Claude CLI

## Overview

Transform claude-agent into an **ACP-to-Claude proxy server** that maintains persistent `claude` CLI processes and translates between ACP protocol and stream-json format.

## Architecture

```
ACP Client ←→ claude-agent (proxy) ←→ persistent claude CLI process
            [ACP protocol]         [stream-json stdin/stdout]
```

## Key Concepts

1. **One process per ACP session** - persistent, not per-message
2. **Protocol proxy** - translate ACP ↔ stream-json bidirectionally
3. **Direct CLI control** - no SDK layer
4. **Session lifecycle mapping** - session create/delete → process spawn/terminate

## Implementation Phases

This issue has been broken into 4 separate focused issues:

1. **Phase 1**: [01-claude-process-manager](01-claude-process-manager.md)
   - Build ClaudeProcessManager and ClaudeProcess
   - Handle process spawning, I/O, lifecycle
   - Standalone module, no dependencies

2. **Phase 2**: [02-protocol-translator](02-protocol-translator.md)
   - Build ProtocolTranslator
   - ACP → stream-json conversion
   - stream-json → ACP conversion

3. **Phase 3**: [03-integrate-with-agent](03-integrate-with-agent.md)
   - Wire ProcessManager into ClaudeClient
   - Hook session lifecycle into agent
   - Update prompt handling

4. **Phase 4**: [04-remove-sdk-dependency](04-remove-sdk-dependency.md)
   - Remove claude-sdk-rs from Cargo.toml
   - Clean up imports and error types
   - Verify tests pass

## CLI Command

```bash
claude \
  --dangerously-skip-permissions \
  --input-format stream-json \
  --output-format stream-json
```

## Benefits

1. **Efficiency**: One process per session vs one per message
2. **State preservation**: Claude CLI maintains context internally
3. **No SDK dependency**: Direct process control
4. **Clear architecture**: We ARE the ACP proxy
5. **Better debugging**: Direct stdin/stdout inspection

## Work Breakdown

- [ ] Phase 1: Process management (01-claude-process-manager)
- [ ] Phase 2: Protocol translation (02-protocol-translator)
- [ ] Phase 3: Integration (03-integrate-with-agent)
- [ ] Phase 4: Remove SDK (04-remove-sdk-dependency)

## References

- Current implementation: `lib/src/claude.rs`, `lib/src/agent.rs`
- Session management: `lib/src/session.rs`
- ACP protocol: `agent_client_protocol` crate
