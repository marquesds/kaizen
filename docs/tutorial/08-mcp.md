# Part 8 — Agents calling Kaizen: MCP

The **`kaizen mcp`** command exposes a stdio MCP server so another agent can list sessions, pull summaries, run metrics, trigger `init`, flush sync **once**, run retro, and manage experiments — without spawning a full shell for each call.

## Start the server

From the **workspace root** you care about (or pass `workspace` per tool):

```bash
cd /path/to/your-project
kaizen mcp
```

Host examples (Cursor, VS Code, Copilot CLI, OpenCode, Goose): [mcp.md](../mcp.md).

## Capabilities tool

Call **`kaizen_capabilities`** first in unfamiliar setups. It is static text that routes you to `kaizen_summary` vs `kaizen_metrics` vs session tools.

## Parity mental model

- **MCP:** ingest, sessions, summary, init, insights, metrics (+ index), sync run/status (run: **once** only), experiments, retro, TUI stub.
- **Shell only:** `doctor`, `guidance`, `gc`, `completions`, `proxy`, `telemetry`.

Cache-first and `refresh` / `all_workspaces` match the CLI. Details: [mcp.md](../mcp.md#behavior-notes).

## Exercise

1. Add `kaizen mcp` to your editor’s MCP config with `cwd` set to a real repo.
2. From a test prompt, call `kaizen_capabilities`, then `kaizen_summary` with `json: true`.
3. Compare output to `kaizen summary --json` in the same directory.

**Next:** [Part 9 — Housekeeping](09-housekeeping.md)
