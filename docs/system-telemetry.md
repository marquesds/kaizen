# System sampler

Opt-in **local-only** samples of the agent process: CPU% and resident memory on a schedule while the session runs, written to `session_samples`. Nothing is sent off machine except through your existing optional sync (same redaction rules as other payloads — samples are in local SQLite only unless you add an exporter that reads them; v1 has no remote schema for this table).

## Enable

```toml
[collect.system_sampler]
enabled = true
sample_ms = 2000
max_samples_per_session = 3600
```

## Hook contract

On **SessionStart**, the hook JSON may include:

- **`pid`** (number) — process to sample; **required** to start the sampler.
- **`ppid`** (number) — reserved for future use (ignored in v1).

Ingest spawns `kaizen __sampler-run --workspace … --session … --pid …` without blocking. See [ingest-contract.md](ingest-contract.md#hook-payload-optional-fields).

## Stop signal

On **Stop**, ingest creates an empty file at:

`~/.kaizen/projects/<slug>/sampler-stop/<session_id>`

The sampler loop checks for this file each iteration and exits. The process also stops if the PID disappears or `max_samples_per_session` is reached.

## Platform support

- **macOS** and **Linux** — supported via `sysinfo` (see `Cargo.toml` features).
- **Windows** — not supported in v1 (sampler is a no-op or skip).

## Privacy

Data stays on disk under the workspace; do not set `pid` to unrelated processes. Document which PID your agent integration sends (often the main IDE or CLI host process).

## Retro

H30 (CPU), H31 (RSS), H32 (long run / many samples) use aggregated per-session maxima. Formal lifecycle: [`specs/system-sampler.qnt`](../specs/system-sampler.qnt).
