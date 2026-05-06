# ADR 007: Project Data in Home Directory

## Status
Accepted

## Context

All per-project artifacts (DB, config, search index, reports, telemetry, backups, sampler stop files) previously lived under `<workspace>/.kaizen/`. This polluted repos, made `.gitignore` maintenance necessary, and caused issues when workspaces were read-only or on network mounts.

## Decision

Move all per-project artifacts to `~/.kaizen/projects/<slug>/` where `<slug>` is the canonical workspace path with `/` replaced by `-` (e.g. `/Users/alice/Projects/my-app` → `Users-alice-Projects-my-app`).

`KAIZEN_HOME` overrides `~/.kaizen`.

Identity (workspace key) remains the canonical absolute path string — unchanged from before.

Auto-migration: on first `workspace::resolve` or `kaizen init`, if `<workspace>/.kaizen/` exists it is moved to the project data dir and a `MIGRATED.txt` marker is left. Conflict (both sides non-empty) logs a warning and skips.

## Consequences

- Repos no longer contain any kaizen data files; `.gitignore` entries for `.kaizen/` become optional.
- Read-only workspaces work fine — data goes to home.
- `KAIZEN_HOME` provides full isolation for tests and multi-user setups.
- Existing `.kaizen/` dirs auto-migrate; users can delete the old dir after `MIGRATED.txt` appears.
