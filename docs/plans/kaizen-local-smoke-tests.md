# Kaizen Local Smoke Tests

Subprocess-oriented smoke coverage for throwaway workspaces. User-facing command
semantics live in [usage.md](../usage.md); MCP mapping lives in
[mcp.md](../mcp.md).

Continuation:

- [Workflow and integration tiers](kaizen-local-smoke-tests-workflows.md)
- [MCP, optional, and TUI tiers](kaizen-local-smoke-tests-mcp.md)

## Pass Criteria

The default expected exit status is `0`. Text output must contain documented
markers. JSON output must parse without stdout contamination.

Set `KAIZEN_BIN` to an absolute binary path. All examples use that variable.

The local persistence model is one SQLite database under
`$KAIZEN_HOME/projects/<slug>/kaizen.db`. No hot log, cold partition, or storage
migration command is part of the smoke surface.

## Version and Isolation

Before smoke, bump the package version when required by release discipline,
build the binary, and update `CHANGELOG.md` for user-visible changes.

Use fresh directories:

```bash
TMP="$(mktemp -d)"
export KAIZEN_HOME="$TMP/kaizen-home"
export WORKDIR="$TMP/ws-main"
export WORKDIR2="$TMP/ws-other"
mkdir -p "$KAIZEN_HOME" "$WORKDIR" "$WORKDIR2"
WORKDIR_CANON="$(cd "$WORKDIR" && pwd -P)"
PROJECT_SLUG="$(printf '%s' "${WORKDIR_CANON#/}" | tr '/' '-')"
export PROJECT_DIR="$KAIZEN_HOME/projects/$PROJECT_SLUG"
NOW_MS="$(($(date +%s) * 1000))"
```

Fresh `KAIZEN_HOME` isolates `machine.db`, project data, daemon files, telemetry,
and config from the developer installation.

## Default Skips

Record these as `SKIP`, not failures:

- OpenClaw host files and real transcript tails.
- Remote PostHog, Datadog, or OTLP traffic.
- Provider and mixed-source reads without configured credentials.
- Real OS transcript paths without fixtures.
- `kaizen eval run` without judge credentials.

Nested `--help` coverage belongs to `tests/cli_help_smoke.rs` and
`tests/cli_help_matrix.inc`.

## G0: Global Preconditions

| ID | Command | Pass |
|---|---|---|
| G0.1 | `$KAIZEN_BIN --version` | Matches package version. |
| G0.2 | `$KAIZEN_BIN --help` | Shows grouped commands. |
| G0.3 | `$KAIZEN_BIN summary` in `$WORKDIR` after init | Exits `0`. |
| G0.4 | `$KAIZEN_BIN --no-daemon summary` | Matches daemon-backed data. |
| G0.5 | `$KAIZEN_BIN summary --workspace "$WORKDIR"` | Works outside workspace cwd. |

## Tier A: Workspace Bootstrap

| ID | Action | Pass |
|---|---|---|
| A1 | `$KAIZEN_BIN init --workspace "$WORKDIR"` | `$PROJECT_DIR/config.toml` and `$PROJECT_DIR/kaizen.db` exist; second run succeeds. |
| A2 | `$KAIZEN_BIN doctor --workspace "$WORKDIR"` | Store opens and project data directory is writable. |
| A3 | Make project data directory read-only, then run doctor | Exits non-zero; restore permissions. |

Skip OpenClaw-specific artifacts unless testing that integration.

## Tier BD: Daemon Lifecycle

| ID | Action | Pass |
|---|---|---|
| BD1 | `$KAIZEN_BIN daemon start --background` | Prints pid, socket, log, and web URL. |
| BD2 | `$KAIZEN_BIN daemon status` | Reports running state and capture health. |
| BD3 | `$KAIZEN_BIN daemon stop` | Stops cleanly; later status reports stopped. |

At least one complete audit should exercise daemon mode. Simpler CI can use
`--no-daemon`.

## Tier B: Ingest and Session Reads

Cursor SessionStart:

```json
{"event":"SessionStart","session_id":"smoke-s1","timestamp_ms":CURRENT_NOW_MS}
```

Claude Code SessionStart:

```json
{"hook_event_name":"SessionStart","session_id":"smoke-s2","timestamp_ms":CURRENT_NOW_MS}
```

| ID | Command | Pass |
|---|---|---|
| B1 | `ingest hook --source cursor` with Cursor JSON | Session appears later. |
| B2 | `ingest hook --source claude` with Claude JSON | Second session appears. |
| B3 | `sessions list` and `sessions list --json` | Expected ids; JSON parses. |
| B4 | `sessions show <id>` | Includes session metadata and `trace_path`. |
| B5 | `sessions tree <id>`, `--depth 2`, and `--json` | Stable text and JSON shapes. |
| B6 | `sessions search "smoke" --limit 10` | Exits without panic. |
| B7 | `search reindex` | `$PROJECT_DIR/search/` is populated. |
| B8 | Repeat search | Uses healthy index. |

## Tier C: Local Aggregates

Keep default `--source local`.

| ID | Commands | Pass |
|---|---|---|
| C1 | `summary`, `--json`, `--refresh` | All succeed; JSON shape matches docs. |
| C1b | `summary --all-workspaces` | Merges two registered workspaces. |
| C2 | `insights`, `--refresh`, `--all-workspaces` | All succeed. |
| C3 | `guidance`, `--days 14`, `--json`, `--refresh` | All succeed. |
| C4 | `metrics --days 7`, `--json`, `--force`, `--refresh`, `--all-workspaces` | All succeed. |
| C5 | `metrics index` and `metrics index --force` | Both succeed. |

For git-specific assertions, initialize and commit inside the disposable
workspace.

## Tier D: Retro, GC, and SQLite Authority

| ID | Command | Pass |
|---|---|---|
| D1 | `retro`, `--dry-run`, `--json`, `--force` | Outputs match documented write behavior. |
| D2 | `gc --days 365` | Succeeds on disposable data. |
| D2b | `gc --vacuum` | Succeeds; slower execution is acceptable. |
| D3 | Add `$PROJECT_DIR/cold/events/legacy.parquet`, then run `summary` | Summary remains SQLite-derived and unchanged. |
| D4 | `$KAIZEN_BIN migrate --help` | Fails as an unknown command. |

Run feedback and experiment checks before GC when fixtures use old timestamps.
