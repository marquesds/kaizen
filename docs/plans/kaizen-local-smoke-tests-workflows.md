# Kaizen Smoke Tests: Workflows

Run [base setup and tiers A-D](kaizen-local-smoke-tests.md) first.

## Tier E: Feedback and Prompts

| ID | Command | Pass |
|---|---|---|
| E1 | `sessions annotate <id> --score 3 --label good --note "smoke"` | Feedback appears on later session show. |
| E2 | `feedback list`, filters, and `--json` | Stable text; JSON parses. |
| E3 | `prompt list` and `--json` | Succeeds, including empty state. |
| E4 | `prompt show` and `prompt diff` when fingerprints exist | Succeeds. |

## Tier F: Experiment Lifecycle

Run this sequence:

1. `exp power --metric tokens_per_session --baseline-n 50`
2. `exp new --name smoke --hypothesis h --change c --metric cost_per_session --bind manual`
3. `exp list` and `exp status <id>`
4. `exp start <id>`
5. `exp tag <id> --session <sid> --variant control`
6. `exp report <id>` and `exp report <id> --json`
7. `exp conclude <id>`
8. `exp archive <id>`

Also check that invalid lifecycle transitions fail predictably. Git and branch
bindings require a disposable repository with commits.

## Tier G: Sync Without a Server

Leave `[sync].endpoint` empty.

| ID | Command | Pass |
|---|---|---|
| G1 | `sync status --workspace "$WORKDIR"` | Stable disabled or empty-outbox result. |
| G2 | `sync run --once --workspace "$WORKDIR"` | Succeeds without network dependency. |

## Tier H: Local HTTP Proxy

| ID | Action | Pass |
|---|---|---|
| H1 | Start `proxy run --listen 127.0.0.1:0` | Process binds and remains alive. |
| H2 | Send malformed or unsupported request | Returns 4xx/502 without crashing. |

Terminate the process and clean up its timeout or signal handler.

## Tier I: File Telemetry

Use the isolated `KAIZEN_HOME` from the base plan. Configure a file exporter in
`$PROJECT_DIR/config.toml` or through the wizard:

```toml
[[telemetry.exporters]]
type = "file"
enabled = true
```

The default output path is `$PROJECT_DIR/telemetry.ndjson`.

| ID | Command | Pass |
|---|---|---|
| I1 | `telemetry print-schema` | Prints schema. |
| I2 | `telemetry print-effective-config` | Prints redacted resolution. |
| I3 | `telemetry configure --type file --path telemetry.ndjson` | Writes one exporter row. |
| I4 | Trigger an exporter write | File appears when a batch exists. |
| I5 | `telemetry tail --no-follow` and `--json` | Succeeds; non-empty lines parse. |

Skip remote push, doctor, and pull checks unless credentials and intended
network access are available.

## Tier J: Completions

Run `completions bash`, `zsh`, `fish`, `elvish`, and `powershell`. Every command
must succeed and print non-empty output.
