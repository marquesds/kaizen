# Setup and Projects

[Back to CLI index](usage.md).

## Selecting a Project

Every command resolves a workspace through one of three mechanisms:

| Flag | Behavior |
|---|---|
| _(none)_ | Uses the current working directory. |
| `--project <NAME>` | Resolves a registered workspace by short name or slug. |
| `--workspace <PATH>` | Uses an explicit absolute path. |

`--project` and `--workspace` are mutually exclusive.

`sessions list`, `summary`, `insights`, and `metrics` accept
`--all-workspaces`. `kaizen load` defaults to all registered workspaces. Kaizen
opens each registered project database separately and merges results. See
[machine-local registry](config.md#machine-local-registry).

After a full transcript rescan, Kaizen may delete sessions older than
`[retention].hot_days`, at most once per 24 hours. Set `hot_days = 0` to disable
automatic pruning; use `kaizen gc` for explicit pruning.

## `kaizen doctor`

Checks version, config paths, store access, and optional hook wiring. It exits
with status `1` if the project data directory is not writable. It does not
modify project data.

## `kaizen init`

Idempotent workspace setup:

| Artifact | Action |
|---|---|
| `~/.kaizen/projects/<slug>/config.toml` | Created if missing. |
| `~/.cursor/hooks.json` | Patched for Cursor lifecycle hooks. |
| `~/.claude/settings.json` | Patched for Claude Code lifecycle hooks. |
| `~/.openclaw/hooks/kaizen-events/handler.ts` | Written for OpenClaw events. |
| `~/.cursor/skills/kaizen-retro/SKILL.md` | Written unless already replaced. |
| `~/.cursor/skills/kaizen-eval/SKILL.md` | Written unless already replaced. |
| `~/.kaizen/projects/<slug>/backup/*.bak` | Created before changing existing host files. |
| Legacy `<workspace>/.kaizen/` | Copied into project data; source remains unchanged. |

Codex, Goose, OpenCode, Copilot, and other supported agents are also ingested
through transcript tails. Re-running `init` is safe. Kaizen never creates,
edits, moves, or deletes files inside the target workspace.
An in-workspace `KAIZEN_HOME` is rejected before setup writes anything.

```bash
kaizen init
kaizen init --deep
```

Normal `init` starts daemon-backed workspace capture. `--deep` also starts
supported loopback proxy tasks. Unsupported provider rewrites remain unchanged
and are reported as partial deep capture.

## `kaizen open`

Starts the local daemon when needed and opens the authenticated loopback
dashboard in the default browser. Use `kaizen open --no-browser` to print the
URL without launching a browser. The command requires daemon mode.

## `kaizen projects`

```bash
kaizen projects
kaizen projects --json
kaizen projects --include-missing
```

The list shows each registered workspace's short name, slug, canonical path,
and status. By default, it hides registry rows whose paths no longer exist.
Use `--include-missing` to inspect stale rows; Kaizen never deletes them
automatically. `kaizen projects list` remains a compatible alias and accepts
the same flags.

`--project`, `--all-workspaces`, daemon reads, and `kaizen load` use existing
paths only. Workspace-scoped commands register paths automatically.

## `kaizen load`

Loads previous local agent sessions from machine transcript stores:

```bash
kaizen load
kaizen load --workspace /repo --json
kaizen load --project kaizen
kaizen sessions load --json
```

Use `load` after installing or upgrading when existing sessions should appear
in reports. Use `sessions list --refresh` when one read command should rescan
before rendering.

## `kaizen outcomes`

```text
kaizen outcomes show <id> [--workspace]
```

This returns a JSON row from `session_outcomes`. It requires
`[collect.outcomes].enabled` and a completed `Stop` hook. See
[outcomes.md](outcomes.md).
