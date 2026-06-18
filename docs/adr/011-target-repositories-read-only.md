# ADR 011: Target Repositories Are Read-Only

## Status
Accepted

## Context

Kaizen observes repositories owned by its users. Earlier releases could move a
legacy `.kaizen/` directory, leave a migration marker, apply guidance edits, or
resolve relative telemetry files inside that repository. Even intentional
automation made adoption risky because running Kaizen could dirty the project.

## Decision

Treat every target repository as a read-only input boundary for Kaizen-owned
file operations.

- Store project data, indexes, reports, backups, and markers under
  `$KAIZEN_HOME/projects/<slug>/`.
- Install host integrations under user-level Cursor, Claude Code, and OpenClaw
  directories.
- Copy legacy `<workspace>/.kaizen/` data into project data. Leave source bytes
  unchanged, reject symlinks and special files, and write
  `LEGACY_IMPORTED.txt` only in project data.
- Keep guidance proposals review-only. Users apply accepted edits themselves.
- Resolve relative telemetry file paths under project data and reject absolute
  exporter paths inside the target repository.
- Reject traversal, symlink, and hard-link aliases that could redirect any
  Kaizen-owned project, home, database, or daemon-runtime write into the target
  repository.

Opt-in outcome commands remain user-configured subprocesses. Their underlying
tooling may create normal ignored build output; Kaizen does not create those
files itself.

## Consequences

- `kaizen init` works with read-only repositories and never dirties Git state.
- Legacy directories require manual deletion after users verify the copied data.
- Guidance loses one-command mutation but gains a clear review boundary.
- Existing applied guidance rows remain readable for storage compatibility.
