// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen telemetry` subcommands: configure, print-effective, doctor, pull, push, schema.

use crate::core::config::{self, ExporterConfig, try_team_salt};
use crate::provider::{PullWindow, from_config as provider_from_config};
use crate::shell::cli::workspace_path;
use crate::shell::scope;
use crate::store::Store;
use crate::store::remote_cache::{RemoteCacheStore, RemotePullState};
use crate::sync::canonical::KAIZEN_SCHEMA_VERSION;
use crate::sync::chunk_events_into_ingest_batches;
use crate::sync::outbound::outbound_event_from_row;
use crate::sync::redact::redact_payload;
use crate::sync::workspace_hash;
use crate::telemetry::{self, DatadogResolved, OtlpResolved, PostHogResolved};
use anyhow::{Context, Result};
use std::io::{BufRead, Write};
use std::path::Path;

/// Interactive: append a PostHog stub row to `~/.kaizen/config.toml` (keys via env or paste).
pub fn cmd_telemetry_configure(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let home = std::env::var("HOME").context("HOME not set")?;
    let p = std::path::PathBuf::from(home).join(".kaizen/config.toml");
    std::fs::create_dir_all(p.parent().unwrap())?;

    println!("Kaizen pluggable telemetry — optional sinks fan-out alongside Kaizen sync.");
    println!(
        "This command appends a `[[telemetry.exporters]]` table to {}.",
        p.display()
    );
    print!("Type `file`, `posthog`, `datadog`, `otlp`, or `dev` (or empty to abort): ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let t = line.trim().to_lowercase();
    if t.is_empty() {
        println!("Aborted.");
        return Ok(());
    }
    let block = match t.as_str() {
        "file" => {
            r#"
[[telemetry.exporters]]
type = "file"
enabled = true
# path = "telemetry.ndjson"   # optional; default .kaizen/telemetry.ndjson under each workspace
"#
        }
        "posthog" => {
            r#"
[[telemetry.exporters]]
type = "posthog"
# project_api_key = "phc_..."  # or set POSTHOG_API_KEY
# host = "https://us.i.posthog.com"
"#
        }
        "datadog" => {
            r#"
[[telemetry.exporters]]
type = "datadog"
# api_key = "..."   # or set DD_API_KEY
# site = "datadoghq.com"
"#
        }
        "otlp" => {
            r#"
[[telemetry.exporters]]
type = "otlp"
# endpoint = "http://127.0.0.1:4318"  # or set OTEL_EXPORTER_OTLP_ENDPOINT
"#
        }
        "dev" => {
            r#"
[[telemetry.exporters]]
type = "dev"
"#
        }
        _ => anyhow::bail!("unknown type (use file, posthog, datadog, otlp, dev)"),
    };

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)?;
    if f.metadata()?.len() > 0 {
        f.write_all(b"\n")?;
    }
    f.write_all(block.as_bytes())?;
    let _ = ws;
    println!(
        "Appended. `file` needs no extra feature; use `--features telemetry-posthog` for PostHog."
    );
    Ok(())
}

/// Redacted: show which env/Toml fields are visible for `telemetry` sinks.
pub fn print_effective_config_text(workspace: Option<&Path>) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(&mut s, "telemetry.fail_open: {}", cfg.telemetry.fail_open).unwrap();
    for (i, e) in cfg.telemetry.exporters.iter().enumerate() {
        match e {
            ExporterConfig::None => writeln!(&mut s, "[{i}] type=none (ignored)").unwrap(),
            ExporterConfig::File { enabled, path } => {
                let p = path
                    .as_deref()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "<workspace>/.kaizen/telemetry.ndjson".into());
                writeln!(&mut s, "[{i}] type=file enabled={enabled} path={p}").unwrap();
            }
            ExporterConfig::Dev { enabled } => {
                writeln!(&mut s, "[{i}] type=dev enabled={enabled}").unwrap();
            }
            ExporterConfig::PostHog { .. } => {
                let line = if let Some(r) = PostHogResolved::from_config(e) {
                    format!(
                        "[{i}] type=posthog host={} key=<redacted len {}>",
                        r.host,
                        r.project_api_key.len()
                    )
                } else {
                    format!(
                        "[{i}] type=posthog (unresolved: set POSTHOG_API_KEY or project_api_key)"
                    )
                };
                writeln!(&mut s, "{line}").unwrap();
            }
            ExporterConfig::Datadog { .. } => {
                let line = if let Some(r) = DatadogResolved::from_config(e) {
                    format!(
                        "[{i}] type=datadog site={} key=<redacted len {}>",
                        r.site,
                        r.api_key.len()
                    )
                } else {
                    format!("[{i}] type=datadog (unresolved: set DD_API_KEY or api_key in TOML)")
                };
                writeln!(&mut s, "{line}").unwrap();
            }
            ExporterConfig::Otlp { .. } => {
                let line = if let Some(r) = OtlpResolved::from_config(e) {
                    format!("[{i}] type=otlp endpoint={}", r.endpoint)
                } else {
                    format!("[{i}] type=otlp (unresolved: OTEL_EXPORTER_OTLP_ENDPOINT)")
                };
                writeln!(&mut s, "{line}").unwrap();
            }
        }
    }
    if cfg.telemetry.exporters.is_empty() {
        writeln!(&mut s, "(no [[telemetry.exporters]] rows)").unwrap();
    }
    Ok(s)
}

pub fn cmd_telemetry_print_effective(workspace: Option<&Path>) -> Result<()> {
    print!("{}", print_effective_config_text(workspace)?);
    Ok(())
}

/// Alias of [`cmd_telemetry_configure`].
pub fn cmd_telemetry_init(workspace: Option<&Path>) -> Result<()> {
    cmd_telemetry_configure(workspace)
}

/// Resolve config, run provider `health` when available, show redacted exporter view.
pub fn cmd_telemetry_doctor(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    println!("telemetry.fail_open: {}", cfg.telemetry.fail_open);
    println!(
        "telemetry.query.cache_ttl_seconds: {}",
        cfg.telemetry.query.cache_ttl_seconds
    );
    match cfg.telemetry.query.provider {
        crate::core::config::QueryAuthority::None => println!("telemetry.query.provider: none"),
        crate::core::config::QueryAuthority::Posthog => {
            println!("telemetry.query.provider: posthog");
        }
        crate::core::config::QueryAuthority::Datadog => {
            println!("telemetry.query.provider: datadog");
        }
    }
    if let Some(p) = provider_from_config(&cfg.telemetry.query) {
        match p.health() {
            Ok(()) => println!("provider health: ok (schema: {})", p.schema_version()),
            Err(e) => eprintln!("provider health: {e}"),
        }
    } else {
        println!("query provider: (not configured or features disabled; pull disabled)");
    }
    println!("\n{}", print_effective_config_text(Some(&ws))?);
    println!("\nOTLP: export only — no query/pull in v1.");
    Ok(())
}

/// Run one page of `pull` and refresh `remote_pull_state` (payload import when APIs are wired).
pub fn cmd_telemetry_pull(workspace: Option<&Path>, days: u32) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let p = provider_from_config(&cfg.telemetry.query)
        .ok_or_else(|| anyhow::anyhow!("set [telemetry.query] provider and credentials"))?;
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let page = p.pull(PullWindow { days }, None)?;
    if !cfg.sync.team_id.trim().is_empty()
        && let Some(ctx) = crate::sync::ingest_ctx(&cfg, ws.to_path_buf())
        && let Some(wh) = crate::sync::smart::workspace_hash_for(&ctx)
    {
        match crate::provider::import_pull_page_to_remote(&store, &cfg.sync.team_id, &wh, &page) {
            Ok(n) if n > 0 => {
                tracing::debug!(n, "remote_events: imported from provider pull (cmd)")
            }
            _ => {}
        }
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let label = match cfg.telemetry.query.provider {
        crate::core::config::QueryAuthority::None => "none",
        crate::core::config::QueryAuthority::Posthog => "posthog",
        crate::core::config::QueryAuthority::Datadog => "datadog",
    };
    store.set_pull_state(&RemotePullState {
        query_provider: label.into(),
        cursor_json: page.next_cursor.unwrap_or_default(),
        last_success_ms: Some(now_ms),
    })?;
    println!("pull: received {} item(s) (page)", page.items.len());
    Ok(())
}

/// Replay stored events in a trailing window through configured telemetry exporters (no Kaizen POST).
pub fn cmd_telemetry_push(
    workspace: Option<&Path>,
    all_workspaces: bool,
    days: u32,
    dry_run: bool,
) -> Result<()> {
    let roots = scope::resolve(workspace, all_workspaces)?;
    let primary = roots
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no workspace roots"))?;
    let cfg = config::load(&primary)?;
    let Some(salt) = try_team_salt(&cfg.sync) else {
        anyhow::bail!(
            "telemetry push requires [sync].team_salt_hex (64 hex chars). \
             The salt hashes session/workspace identifiers and drives payload redaction — \
             not only for cloud sync."
        );
    };
    let registry = telemetry::load_exporters(&cfg.telemetry, primary.as_path());
    if registry.is_empty() {
        anyhow::bail!(
            "no telemetry exporters to push to: add [[telemetry.exporters]] (e.g. type = \"file\" \
             needs no extra feature; PostHog/Datadog/OTLP need build features); see \
             `kaizen telemetry print-effective-config`."
        );
    }
    let fail_open = cfg.telemetry.fail_open;
    let team_id = cfg.sync.team_id.clone();
    let end_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let start_ms = end_ms.saturating_sub((days as u64).saturating_mul(86_400_000));

    let mut total_events: u64 = 0;
    let mut total_batches: u64 = 0;

    for root in &roots {
        let store = Store::open(&root.join(".kaizen/kaizen.db"))?;
        let ws_key = root.to_string_lossy().to_string();
        let rows = store.retro_events_in_window(&ws_key, start_ms, end_ms)?;
        let wh = workspace_hash(&salt, root.as_path());
        let outbound: Vec<_> = rows
            .into_iter()
            .map(|(session, ev)| {
                let mut o = outbound_event_from_row(&ev, &session, &salt);
                redact_payload(&mut o.payload, root.as_path(), &salt);
                o
            })
            .collect();
        let n = outbound.len() as u64;
        total_events += n;
        let batches = chunk_events_into_ingest_batches(team_id.clone(), wh, outbound, &cfg.sync)?;
        let bcount = batches.len() as u64;
        total_batches += bcount;
        if dry_run {
            eprintln!(
                "telemetry push (dry-run): {} — {} event(s), {} batch(es)",
                root.display(),
                n,
                bcount
            );
            continue;
        }
        for batch in batches {
            registry
                .fan_out(fail_open, &batch)
                .with_context(|| format!("telemetry fan-out ({})", batch.kind_name()))?;
        }
        eprintln!(
            "telemetry push: {} — sent {} event(s) in {} batch(es)",
            root.display(),
            n,
            bcount
        );
    }

    eprintln!(
        "telemetry push: total {} event(s), {} batch(es) across {} workspace(s){}",
        total_events,
        total_batches,
        roots.len(),
        if dry_run { " (dry-run)" } else { "" }
    );
    Ok(())
}

/// Example JSON for canonical per-item export names (ingest + third-party mappers).
pub fn cmd_telemetry_print_schema() -> Result<()> {
    let v = serde_json::json!({
        "kaizen_schema_version": KAIZEN_SCHEMA_VERSION,
        "event_names": [
            "kaizen.event",
            "kaizen.tool_span",
            "kaizen.repo_snapshot_chunk",
            "kaizen.workspace_fact_snapshot"
        ],
        "note": "Full shapes: see sync::canonical::CanonicalItem and expand_ingest_batch (tests include golden JSON).",
    });
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}
