# Part 1 — Setup: install, init, doctor

## Before you start

You need the `kaizen` binary on your `PATH` and a **project directory** where you use an agent. If you have not installed yet, follow [install.md](../install.md).

## Why init first

Kaizen does not replace your agent. It **observes** it: transcript files on disk, optional **hooks** for lower-latency events, and (later) an optional HTTP proxy. `kaizen init` is the one command that wires Cursor and Claude Code globally — it writes to `~/.cursor/hooks.json` and `~/.claude/settings.json` so every workspace is covered from a single run.

## Run init

You can run `kaizen init` from any directory — once is enough for all workspaces:

```bash
kaizen init
```

You should see paths like `~/.kaizen/projects/<slug>/config.toml`, `~/.cursor/hooks.json`, `~/.claude/settings.json`, and `~/.cursor/skills/kaizen-retro/SKILL.md` described in the output. If files already existed, Kaizen may **skip** or **patch** idempotently; backups land under `~/.kaizen/projects/<slug>/backup/`.

**Insight:** Other agents (Codex, Goose, OpenCode, Copilot) are picked up via **transcript tail** configured under `~/.kaizen/config.toml` `[sources]` — not by extra files `init` writes today.

## Run doctor

```bash
kaizen doctor
```

Use this after init or in CI. Exit code **1** means a hard problem (for example store open failure or `.kaizen/` not writable). Partial hook wiring is reported in text but is not always a hard failure — follow the printed hint to re-run `kaizen init` if needed.

## Exercise

1. Run `kaizen init` once; confirm `~/.cursor/hooks.json` and `~/.claude/settings.json` were written or patched.
2. Run `kaizen doctor` and read the hook section.
3. Run `kaizen projects list` to see your registered workspaces with their short names:

   ```bash
   kaizen projects list
   # NAME        SLUG                                PATH
   # my-app      Users-alice-Projects-my-app         /Users/alice/Projects/my-app
   # kaizen      Users-alice-Projects-kaizen         /Users/alice/Projects/kaizen
   ```

   You can now use `--project <NAME>` from any directory:

   ```bash
   kaizen summary --project my-app
   ```

4. Optional: run a short agent session, then continue to [Part 2](02-observe.md).

**Next:** [Part 2 — Observe](02-observe.md)
