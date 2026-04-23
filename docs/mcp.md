# kaizen MCP (stdio)

Run the [Model Context Protocol](https://modelcontextprotocol.io) server for **full CLI parity** from agents (Cursor, Claude Code, Goose, OpenCode, GitHub Copilot, and other MCP hosts) without shelling to `kaizen`.

Tier-1 session tailing for Goose, OpenCode, Copilot CLI, and VS Code Copilot Chat runs automatically during `kaizen sessions list`, `kaizen summary`, and related commands (see workspace `.kaizen/config.toml` `[sources.tail]` to disable individual sources).

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

## Goose

Register `kaizen mcp` as an MCP extension per [Goose MCP documentation](https://block.github.io/goose/docs/mcp/). Use the **project root** as the server working directory when the host allows it (or pass `workspace` on each tool call). Extension file format and config location depend on your Goose version—follow the official guide for `command` / `args` / environment.

## OpenCode

In `~/.config/opencode/opencode.json` (or project `opencode.json`), add a **local** MCP server per [OpenCode MCP servers](https://dev.opencode.ai/docs/mcp-servers/):

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "kaizen": {
      "type": "local",
      "command": ["kaizen", "mcp"],
      "enabled": true
    }
  }
}
```

Run OpenCode from the project directory, or rely on per-tool `workspace` arguments.

## GitHub Copilot (VS Code)

VS Code stores MCP servers in **`mcp.json`** (workspace: `.vscode/mcp.json`, or user profile via **MCP: Open User Configuration**). See [MCP configuration in VS Code](https://code.visualstudio.com/docs/copilot/customization/mcp-servers) and the [MCP file format](https://code.visualstudio.com/docs/copilot/reference/mcp-configuration). Example:

```json
{
  "servers": {
    "kaizen": {
      "type": "stdio",
      "command": "kaizen",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

Use `${workspaceFolder}` in `command` / `args` / `env` if needed. Pass `workspace` on tool calls when the server is not started with the repo as cwd.

## GitHub Copilot CLI

Copilot CLI reads MCP definitions from `~/.copilot/mcp-config.json` ([config dir reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference)). Example:

```json
{
  "mcpServers": {
    "kaizen": {
      "command": "kaizen",
      "args": ["mcp"]
    }
  }
}
```

If you use `COPILOT_HOME` or `--config-dir`, place the file under that directory instead.

## Disabling tier-1 sources

In `.kaizen/config.toml`:

```toml
[sources.tail]
goose = true
opencode = true
copilot_cli = true
copilot_vscode = true
```

Set any value to `false` to skip that agent’s local scan (useful if a VS Code workspace storage walk is too slow).

## Tools

| Tool | CLI equivalent | Notes |
|------|----------------|--------|
| `kaizen_capabilities` | (no CLI; static text) | Read first: which tool to use for cost rollups vs repo metrics, sessions, retro, etc. |
| `kaizen_ingest_hook` | `kaizen ingest hook` | Pass hook JSON in `payload` (not stdin). `source`: `cursor` or `claude`. |
| `kaizen_sessions_list` | `kaizen sessions list` | Optional `json: true` matches `kaizen sessions list --json`. |
| `kaizen_session_show` | `kaizen sessions show` | `id` + optional `workspace`. |
| `kaizen_summary` | `kaizen summary` | Optional `json: true` matches `kaizen summary --json`. |
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
