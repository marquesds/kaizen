# Usage

Kaizen groups commands by workflow. Run `kaizen --help` for the installed
command tree and a command's `--help` for every flag.

All workspace-scoped commands accept `--workspace <path>` or
`--project <name>`; the flags are mutually exclusive. Without either flag,
Kaizen uses the current working directory.

`kaizen init` starts a local daemon for capture and supported read/write paths.
Use `--no-daemon` or `KAIZEN_DAEMON=0` for direct SQLite mode. Both modes use
the same project database. See [daemon.md](daemon.md).

Cache-first reads use the local SQLite database without rescanning agent
transcripts. Pass `--refresh` to ingest recently changed transcript tails before
rendering. Refresh work is bounded; it does not replay all historical files.

## Reference

| Area | Commands | Reference |
|---|---|---|
| Setup | `doctor`, `init`, `open`, `projects`, `load`, `outcomes` | [Setup and projects](usage-setup.md) |
| Observe | `open`, `sessions`, `query`, `summary`, `insights`, `guidance`, `metrics`, `tui`, `retro` | [Observe and report](usage-observe.md) |
| Operate | `upgrade`, `gc`, `completions`, `proxy`, `ingest`, `sync`, `mcp` | [Operate and integrate](usage-operate.md) |
| Telemetry | `telemetry configure`, `test`, `doctor`, `pull`, `push`, `tail` | [Telemetry](usage-telemetry.md) |
| Improve | `exp`, `cases`, `rules`, `alerts`, `review`, `eval`, `prompt`, `feedback` | [Experiments and evaluation](usage-improve.md) |

## Common Reads

```bash
kaizen sessions list
kaizen sessions show <id>
kaizen sessions search "deadlock"
kaizen open
kaizen summary
kaizen metrics
kaizen retro --dry-run
```

## Common Operations

```bash
kaizen init
kaizen open
kaizen doctor
kaizen load
kaizen gc
kaizen upgrade
```

Storage is SQLite-only. Current releases do not expose `kaizen migrate` and do
not read legacy hot-log or cold-partition files. See
[ADR 010](adr/010-sqlite-only-local-storage.md).
