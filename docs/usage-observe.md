# Observe and Report

[Back to CLI index](usage.md).

These commands are cache-first. Pass `--refresh` when they should rescan local
transcripts before rendering. With `--source provider|mixed`, refresh can also
refresh a configured remote provider cache.

## `kaizen sessions`

```bash
kaizen sessions list
kaizen sessions list --json
kaizen sessions list --limit 20
kaizen sessions list --limit 0
kaizen sessions list --refresh
kaizen sessions list --all-workspaces
kaizen sessions show <id>
kaizen sessions tree <id>
kaizen sessions tree <id> --depth 3
kaizen sessions tree <id> --json
kaizen search "deadlock" --since 7d --agent claude-code --limit 50
kaizen search 'path:src/store/sqlite/mod.rs' --kind tool_use
kaizen search 'skill:caveman AND tokens_total:>5000'
```

`sessions show` prints one session row. Use `kaizen tui` for turns, tools, and
live tail.

`sessions tree` renders the nested tool-span tree. Each node shows tool, status,
and subtree cost. Spans consuming more than 40% of session cost are flagged.
Text output shows a placeholder when no spans exist; JSON returns `[]`.

`search` uses the rebuildable Tantivy index at
`~/.kaizen/projects/<slug>/search/`. It indexes redacted event text. Payload
bodies remain in SQLite and are not copied into the index. Rebuild with:

```bash
kaizen search reindex
```

`kaizen sessions search` remains a compatible alias. Because `reindex` names
the maintenance subcommand, search for that literal with
`kaizen search -- reindex`.

Known fields such as `tool:bash`, `tokens_total:>5000`, and
`feedback_label:bad` use the local structured trace query engine.

## `kaizen query`

Structured trace query over local SQLite events. Version 1 supports `AND` terms
and these fields: `agent`, `model`, `kind`, `tool`, `path`, `skill`,
`tokens_total`, `cost_usd`, `eval_score`, `feedback_label`, `prompt`, `status`,
and `span_kind`.

```bash
kaizen query 'tool:bash AND tokens_total:>5000'
kaizen query 'feedback_label:bad' --since 30d --json
```

## `kaizen summary`

Rolls up session count, total USD, agents, and models.

```bash
kaizen summary --json
kaizen summary --refresh
kaizen summary --all-workspaces
```

`summary` sums `cost_usd_e6` on stored events. Transcript ingest estimates cost
from token usage when available; proxy `Cost` events remain authoritative.
After ingest logic changes, run `kaizen summary --refresh` to backfill the
workspace.

JSON includes `workspace`, `stats`, `cost_usd`, optional `cost_note`, and,
when metrics data exists, `hotspot` and `slowest_tool`. Multi-workspace output
also includes `workspaces`.

<a id="cost-shows-zero"></a>

### When Cost Rollup Is Zero

If session count is positive but cost is `$0.00`, stored events have no
`cost_usd_e6`. Cursor transcripts may omit token usage. Claude or Codex
transcripts, hooks with `total_cost_usd`, and Kaizen proxy `Cost` events can
provide cost data. Run `kaizen summary --refresh` after changing ingest.

## `kaizen insights`

Shows activity by day, top tools, recent sessions, and a guidance teaser.

```bash
kaizen insights
kaizen insights --refresh
kaizen insights --all-workspaces
```

## `kaizen guidance`

Reports observed skill and Cursor-rule usage, active-session share, relative
cost, and on-disk inventory.

```bash
kaizen guidance
kaizen guidance --days 14
kaizen guidance --json
kaizen guidance --refresh
kaizen guidance score --days 30 --min-sessions 30
kaizen guidance propose --artifact skill:tdd
kaizen guidance propose --artifact rule:style --apply
kaizen guidance candidates list
kaizen guidance candidates validate <candidate_id>
```

`guidance score` evaluates stored outcomes, feedback, cost, tokens, and repeated
tool loops with a deterministic train/validation split. `guidance propose`
creates one candidate and, with `--apply`, backs up that artifact under
`~/.kaizen/projects/<slug>/backup/guidance/<candidate_id>/`. Validation uses the
candidate's prompt-bound experiment.

## `kaizen metrics`

```bash
kaizen metrics --days 7
kaizen metrics --json
kaizen metrics --all-workspaces
kaizen metrics index --force
```

The index command rebuilds the repository snapshot and GraphQLite sidecar.

## `kaizen tui`

Starts the Ratatui live session browser with list, detail, tree, and live-tail
views.

## `kaizen open`

Starts the local daemon when needed and opens the read-only Web dashboard.
Use `--no-browser` when another process will open the printed URL.

## `kaizen retro`

Writes `~/.kaizen/projects/<slug>/reports/<iso-week>.md`.

```bash
kaizen retro --days 7
kaizen retro --dry-run
kaizen retro --json
kaizen retro --force
```

See [retro.md](retro.md) and [retro-tuning.md](retro-tuning.md).
