# Part 1 — Setup: install, init, doctor

## Before you start

You need the `kaizen` binary on your `PATH` and a **project directory** where you use an agent. If you have not installed yet, follow [install.md](../install.md).

## Why init first

Kaizen does not replace your agent. It **observes** it: transcript files on disk, optional **hooks** for lower-latency events, and (later) an optional HTTP proxy. `kaizen init` is the one command that prepares your repo so those paths exist and Cursor + Claude Code hooks point at `kaizen ingest hook`.

## Run init

From **your** project root (not necessarily the Kaizen source tree):

```bash
cd /path/to/your-project
kaizen init
```

You should see paths like `.kaizen/config.toml`, `.cursor/hooks.json`, `.claude/settings.json`, and `.cursor/skills/kaizen-retro/SKILL.md` described in the output. If files already existed, Kaizen may **skip** or **patch** idempotently; backups land under `.kaizen/backup/`.

**Insight:** Other agents (Codex, Goose, OpenCode, Copilot) are picked up via **transcript tail** configured under `~/.kaizen/config.toml` `[sources]` — not by extra files `init` writes today.

## Run doctor

```bash
kaizen doctor
```

Use this after init or in CI. Exit code **1** means a hard problem (for example store open failure or `.kaizen/` not writable). Partial hook wiring is reported in text but is not always a hard failure — follow the printed hint to re-run `kaizen init` if needed.

## Exercise

1. Run `kaizen init` in a repo; list `.kaizen/` and confirm `config.toml` exists.
2. Run `kaizen doctor` and read the hook section.
3. Optional: run a short agent session in that repo, then continue to [Part 2](02-observe.md).

**Next:** [Part 2 — Observe](02-observe.md)
