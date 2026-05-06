# ADR 008: Host Integration in User Home Directory

## Status
Accepted

## Context

`kaizen init` previously wrote hook files to each workspace's `.cursor/` and `.claude/`
directories. This required running `kaizen init` in every repo and left tool configuration
scattered across repos, creating unnecessary `.gitignore` noise and failing on read-only mounts.

## Decision

Write Cursor hooks to `~/.cursor/hooks.json` and Claude Code hooks to `~/.claude/settings.json`.
Skills (`kaizen-retro`, `kaizen-eval`) land under `~/.cursor/skills/`.

## Consequences

- One `kaizen init` anywhere wires all workspaces automatically.
- Repos stay clean — no AI tool config files committed.
- Legacy workspace-local files (from old installs) are harmless but surfaced by `kaizen doctor`.
- Cursor fires both `~/.cursor/hooks.json` and any project-local `hooks.json`; kaizen ingest
  dedupes by event ID.
