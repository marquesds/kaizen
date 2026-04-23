# kaizen MCP (stdio)

Run the [Model Context Protocol](https://modelcontextprotocol.io) server for **full CLI parity** from agents (e.g. Cursor, Claude Code) without shelling to `kaizen`.

## Quint specification

Behavioral invariants for MCP-specific edges (TUI stub, `sync run` once vs continuous) are model-checked in [specs/mcp-server.qnt](specs/mcp-server.qnt) and replayed in CI via `quint-connect` in [`tests/spec/mcp_server.rs`](../tests/spec/mcp_server.rs). Run `quint typecheck specs/mcp-server.qnt` (see [../CONTRIBUTING.md](../CONTRIBUTING.md) for the pinned Quint version) after editing the spec.

## Start

```bash
kaizen mcp
```

The process speaks MCP over **stdio** (JSON-RPC). Use the workspace directory you care about as the process **current working directory** when the host spawns the server, or pass `workspace` on each tool call.

## Cursor

Add a server entry, for example:

```json
{
  "mcpServers": {
    "kaizen": {
      "command": "kaizen",
      "args": ["mcp"],
      "cwd": "${workspaceFolder}"
    }
  }
}
```

Adjust `command` to an absolute path if `kaizen` is not on `PATH`. Optional: set `"env": { "RUST_LOG": "info" }` for tracing from the `tracing` stack.

## Tools

| Tool | CLI equivalent | Notes |
|------|----------------|--------|
| `kaizen_ingest_hook` | `kaizen ingest hook` | Pass hook JSON in `payload` (not stdin). `source`: `cursor` or `claude`. |
| `kaizen_sessions_list` | `kaizen sessions list` | |
| `kaizen_session_show` | `kaizen sessions show` | `id` + optional `workspace`. |
| `kaizen_summary` | `kaizen summary` | |
| `kaizen_tui` | `kaizen tui` | Not runnable over MCP; returns a structured “use CLI” payload with `is_error` semantics. |
| `kaizen_init` | `kaizen init` | Writes/updates workspace files, same as CLI. |
| `kaizen_insights` | `kaizen insights` | |
| `kaizen_metrics` | `kaizen metrics` | `days`, `json`, `force`, `workspace`. |
| `kaizen_metrics_index` | `kaizen metrics index` | |
| `kaizen_sync_run` | `kaizen sync run` | **Only `once: true` is supported** (default). Continuous sync must use a real shell / service. |
| `kaizen_sync_status` | `kaizen sync status` | |
| `kaizen_exp_new` | `kaizen exp new` | Same long options as the CLI. |
| `kaizen_exp_list` | `kaizen exp list` | |
| `kaizen_exp_status` | `kaizen exp status` | |
| `kaizen_exp_tag` | `kaizen exp tag` | |
| `kaizen_exp_report` | `kaizen exp report` | `json` flag supported. |
| `kaizen_exp_conclude` | `kaizen exp conclude` | |
| `kaizen_retro` | `kaizen retro` | Set `json: true` for the same `Report` JSON as `kaizen retro --json`. |

## Behavior notes

- **Workspace**: most tools accept optional `workspace` (string path). If omitted, the server uses the process current directory, matching CLI defaults.
- **Blocking work** is run on a blocking thread pool so the async MCP runtime is not starved; long `retro` or metrics runs may take time.
- **Version** in the MCP `initialize` response is the built-in string configured for the server (keep in sync with releases when using strict client checks).
