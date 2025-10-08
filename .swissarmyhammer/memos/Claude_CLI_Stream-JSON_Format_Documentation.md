# Claude CLI Stream-JSON Format

## Command Line
```bash
claude -p --input-format stream-json --output-format stream-json --verbose --dangerously-skip-permissions
```

## Input Format (stdin to claude)

**Simple user message:**
```json
{"type":"user","message":{"role":"user","content":"What is 2+2?"}}
```

**Tool result:**
```json
{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_123","type":"tool_result","content":[{"type":"text","text":"file contents here"}]}]}}
```

## Output Format (stdout from claude)

**1. System initialization:**
```json
{"type":"system","subtype":"init","cwd":"/path","session_id":"uuid","tools":[...],"model":"...","permissionMode":"default",...}
```

**2. Assistant response (with tool use):**
```json
{"type":"assistant","message":{"model":"...","id":"msg_...","type":"message","role":"assistant","content":[{"type":"tool_use","id":"toolu_123","name":"read_file","input":{"path":"foo.rs"}}],"stop_reason":null,...},"session_id":"uuid"}
```

**3. Tool result (WE send this):**
After executing tool locally, we send:
```json
{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_123","type":"tool_result","content":[{"type":"text","text":"execution result"}]}]}}
```

**4. Final result metadata:**
```json
{"type":"result","subtype":"success","is_error":false,"duration_ms":2845,"num_turns":1,"result":"...","session_id":"uuid","total_cost_usd":0.114,...}
```

## Key Points

1. **Input messages** must have `type` and `message.role`
2. **Tool execution** happens in our code - claude just requests tools
3. **Tool results** are sent back as user messages with `tool_result` content
4. **`--verbose` is required** for stream-json output format
5. **Permissions** - use `--dangerously-skip-permissions` since we provide ACP permission layer

## Workflow

```
User input → We send {"type":"user",...}
              ↓
Claude processes → Sends {"type":"assistant",...} with tool_use
              ↓
We execute tool locally (NOT claude)
              ↓
We send {"type":"user",...} with tool_result
              ↓
Claude continues → Sends {"type":"assistant",...} with response
              ↓
Claude finishes → Sends {"type":"result",...}
```

## Testing

```bash
# Simple query
echo '{"type":"user","message":{"role":"user","content":"2+2"}}' | \
  claude -p --input-format stream-json --output-format stream-json --verbose

# With tool use
echo '{"type":"user","message":{"role":"user","content":"List files"}}' | \
  claude -p --input-format stream-json --output-format stream-json --verbose --dangerously-skip-permissions
```
