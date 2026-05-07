// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen telemetry` subcommands: configure, print-effective, doctor, pull, push, schema.

use crate::core::config::{self, ExporterConfig, effective_redaction_salt};
use crate::core::paths::kaizen_dir;
use crate::provider::{PullWindow, TelemetryQueryProvider, from_config as provider_from_config};
use crate::shell::cli::workspace_path;
use crate::shell::scope;
use crate::store::Store;
use crate::store::remote_cache::{RemoteCacheStore, RemotePullState};
use crate::sync::IngestExportBatch;
use crate::sync::canonical::KAIZEN_SCHEMA_VERSION;
use crate::sync::outbound::{EventsBatchBody, OutboundEvent, outbound_event_from_row};
use crate::sync::redact::redact_payload;
use crate::sync::smart::outbound_tool_span;
use crate::sync::workspace_hash;
use crate::sync::{chunk_events_into_ingest_batches, chunk_tool_spans_into_ingest_batches};
use crate::telemetry::{self, DatadogResolved, OtlpResolved, PostHogResolved};
use anyhow::{Context, Result};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ConfigureOptions {
    pub exporter_type: Option<String>,
    pub path: Option<PathBuf>,
    pub api_key: Option<String>,
    pub site: Option<String>,
    pub host: Option<String>,
    pub endpoint: Option<String>,
    pub non_interactive: bool,
}

/// Validating wizard: prompt for missing creds (or read from env / flags), `health`-check the
/// resolved provider before touching `~/.kaizen/config.toml`, then append the exporter,
/// idempotently set `[telemetry.query].provider` so `pull` works without extra config, and
/// ensure a redaction salt exists. Failure to validate aborts with a clear error and writes
/// nothing. Re-running for the same exporter type + key field is a no-op (no duplicate row).
pub fn cmd_telemetry_configure(workspace: Option<&Path>, options: ConfigureOptions) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let home = kaizen_dir().ok_or_else(|| anyhow::anyhow!("KAIZEN_HOME / HOME unset"))?;
    let cfg_path = home.join("config.toml");
    std::fs::create_dir_all(&home)?;

    println!("Kaizen telemetry — optional sinks fan-out alongside Kaizen sync.");
    let t = resolve_exporter_type(&options)?;
    if t.is_empty() {
        println!("Aborted.");
        return Ok(());
    }

    let block = match t.as_str() {
        "file" => file_exporter_block(options.path.as_deref()),
        "dev" => "\n[[telemetry.exporters]]\ntype = \"dev\"\n".to_string(),
        "datadog" => configure_datadog(&options)?,
        "posthog" => configure_posthog(&options)?,
        "otlp" => configure_otlp(&options)?,
        _ => anyhow::bail!("unknown type (use file, posthog, datadog, otlp, dev)"),
    };

    let existing = std::fs::read_to_string(&cfg_path).unwrap_or_default();
    if exporter_already_present(&existing, &t) {
        println!(
            "Skipped: a `[[telemetry.exporters]]` row of type `{t}` already exists in {}. \
             Edit the file directly to change credentials.",
            cfg_path.display()
        );
    } else {
        append_block(&cfg_path, &block)?;
    }
    ensure_query_authority(&cfg_path, &t)?;

    let cfg = config::load(&ws)?;
    let _ = effective_redaction_salt(&cfg.sync, &home).context(
        "ensure redaction salt (configured `[sync].team_salt_hex` or auto-generated `local_salt.hex`)",
    )?;
    println!("Wrote {}.", cfg_path.display());
    println!("Next: `kaizen telemetry test` to send one synthetic event to every configured sink.");
    Ok(())
}

/// True when the file already contains a `[[telemetry.exporters]]` row whose `type = "<t>"`.
/// Cheap line scan rather than full TOML parse: keeps the wizard side-effect-free if a user
/// hand-edited the file with comments/whitespace we cannot round-trip.
pub(crate) fn exporter_already_present(toml_text: &str, t: &str) -> bool {
    let mut in_exporter_block = false;
    let needle = format!("type = \"{t}\"");
    for line in toml_text.lines() {
        let l = line.trim();
        if l.starts_with("[[telemetry.exporters]]") {
            in_exporter_block = true;
            continue;
        }
        if l.starts_with('[') {
            in_exporter_block = false;
            continue;
        }
        if in_exporter_block && l == needle {
            return true;
        }
    }
    false
}

/// Append `[telemetry.query] provider = "<authority>"` only if the file has no `[telemetry.query]`
/// table yet. Never overrides an existing user choice; only sets one for `posthog` / `datadog`.
fn ensure_query_authority(path: &Path, t: &str) -> Result<()> {
    let authority = match t {
        "datadog" => "datadog",
        "posthog" => "posthog",
        _ => return Ok(()),
    };
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == "[telemetry.query]") {
        return Ok(());
    }
    let block = format!("\n[telemetry.query]\nprovider = \"{authority}\"\n");
    append_block(path, &block)
}

fn resolve_exporter_type(opts: &ConfigureOptions) -> Result<String> {
    if let Some(t) = &opts.exporter_type {
        return Ok(t.trim().to_lowercase());
    }
    if opts.non_interactive {
        anyhow::bail!("--non-interactive requires --type=<file|posthog|datadog|otlp|dev>");
    }
    print!("Type `file`, `posthog`, `datadog`, `otlp`, or `dev` (empty to abort): ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_lowercase())
}

fn configure_datadog(opts: &ConfigureOptions) -> Result<String> {
    let api_key = read_secret(
        "Datadog API key (DD_API_KEY, 32 hex chars — NOT the `ddapp_*` Application Key; \
         create one at Org Settings > API Keys)",
        opts.api_key.clone(),
        "DD_API_KEY",
        opts.non_interactive,
    )?;
    if let Some(rejected) = reject_obvious_app_key(&api_key) {
        anyhow::bail!("{rejected}");
    }
    let site = read_value(
        "Datadog site",
        opts.site.clone(),
        "DD_SITE",
        Some("datadoghq.com".into()),
        opts.non_interactive,
    )?;
    health_check_datadog(&api_key, &site).context(
        "Datadog credentials rejected (DD-API-KEY /api/v1/validate failed); not writing TOML",
    )?;
    Ok(datadog_block(&api_key, &site))
}

fn configure_posthog(opts: &ConfigureOptions) -> Result<String> {
    let key = read_secret(
        "PostHog project API key (phc_...)",
        opts.api_key.clone(),
        "POSTHOG_API_KEY",
        opts.non_interactive,
    )?;
    let host = read_value(
        "PostHog host",
        opts.host.clone(),
        "POSTHOG_HOST",
        Some("https://us.i.posthog.com".into()),
        opts.non_interactive,
    )?;
    health_check_posthog(&host).context("PostHog host unreachable; not writing TOML")?;
    Ok(format!(
        "\n[[telemetry.exporters]]\ntype = \"posthog\"\nproject_api_key = \"{}\"\nhost = \"{}\"\n",
        key.replace('\\', "\\\\").replace('"', "\\\""),
        host.replace('\\', "\\\\").replace('"', "\\\""),
    ))
}

fn configure_otlp(opts: &ConfigureOptions) -> Result<String> {
    let endpoint = read_value(
        "OTLP endpoint",
        opts.endpoint.clone(),
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        Some("http://127.0.0.1:4318".into()),
        opts.non_interactive,
    )?;
    Ok(format!(
        "\n[[telemetry.exporters]]\ntype = \"otlp\"\nendpoint = \"{}\"\n",
        endpoint.replace('\\', "\\\\").replace('"', "\\\""),
    ))
}

/// Local sanity check: DD Application Keys start with `ddapp_`; sending one as `DD-API-KEY`
/// always 403s. Catch the mistake before the network round-trip with a hint that names both
/// key types so the user can tell them apart.
pub(crate) fn reject_obvious_app_key(value: &str) -> Option<&'static str> {
    if value.starts_with("ddapp_") {
        Some(
            "looks like a Datadog Application Key (`ddapp_*`); the wizard needs the API Key \
             (32 hex chars). Generate one at Org Settings > API Keys, then rerun.",
        )
    } else {
        None
    }
}

fn datadog_block(api_key: &str, site: &str) -> String {
    format!(
        "\n[[telemetry.exporters]]\ntype = \"datadog\"\napi_key = \"{}\"\nsite = \"{}\"\n",
        api_key.replace('\\', "\\\\").replace('"', "\\\""),
        site.replace('\\', "\\\\").replace('"', "\\\""),
    )
}

fn append_block(path: &Path, block: &str) -> Result<()> {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    if f.metadata()?.len() > 0 {
        f.write_all(b"\n")?;
    }
    f.write_all(block.as_bytes())?;
    Ok(())
}

fn read_secret(
    prompt: &str,
    flag: Option<String>,
    env_key: &str,
    non_interactive: bool,
) -> Result<String> {
    if let Some(v) = flag.filter(|s| !s.is_empty()) {
        return Ok(v);
    }
    if let Ok(v) = std::env::var(env_key)
        && !v.is_empty()
    {
        return Ok(v);
    }
    if non_interactive {
        anyhow::bail!("missing {env_key}: set the env var or pass --api-key");
    }
    print!("{prompt}: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let v = line.trim().to_string();
    if v.is_empty() {
        anyhow::bail!("{env_key} is required");
    }
    Ok(v)
}

fn read_value(
    prompt: &str,
    flag: Option<String>,
    env_key: &str,
    default: Option<String>,
    non_interactive: bool,
) -> Result<String> {
    if let Some(v) = flag.filter(|s| !s.is_empty()) {
        return Ok(v);
    }
    if let Ok(v) = std::env::var(env_key)
        && !v.is_empty()
    {
        return Ok(v);
    }
    if non_interactive {
        return default.ok_or_else(|| anyhow::anyhow!("missing {env_key}; set env or pass flag"));
    }
    let hint = default
        .as_deref()
        .map(|d| format!(" [{d}]"))
        .unwrap_or_default();
    print!("{prompt}{hint}: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let v = line.trim().to_string();
    if v.is_empty() {
        return default.ok_or_else(|| anyhow::anyhow!("{env_key} is required"));
    }
    Ok(v)
}

fn health_check_datadog(api_key: &str, site: &str) -> Result<()> {
    let r = DatadogResolved {
        site: site.to_string(),
        api_key: api_key.to_string(),
        app_key: None,
    };
    #[cfg(feature = "telemetry-datadog")]
    {
        let c = crate::provider::datadog::DatadogQueryClient::new(&r);
        c.health()
    }
    #[cfg(not(feature = "telemetry-datadog"))]
    {
        let _ = &r;
        anyhow::bail!("rebuild with `--features telemetry-datadog` to validate Datadog");
    }
}

fn health_check_posthog(host: &str) -> Result<()> {
    let r = PostHogResolved {
        host: host.to_string(),
        project_api_key: String::new(),
    };
    #[cfg(feature = "telemetry-posthog")]
    {
        let c = crate::provider::posthog::PostHogQueryClient::new(&r);
        c.health()
    }
    #[cfg(not(feature = "telemetry-posthog"))]
    {
        let _ = &r;
        anyhow::bail!("rebuild with `--features telemetry-posthog` to validate PostHog");
    }
}

fn file_exporter_block(path: Option<&Path>) -> String {
    let mut block = String::from(
        r#"
[[telemetry.exporters]]
type = "file"
enabled = true
"#,
    );
    if let Some(path) = path {
        use std::fmt::Write as _;
        let path = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(&mut block, "path = \"{path}\"").unwrap();
    } else {
        block.push_str(
            "# path = \"telemetry.ndjson\"   # optional; default .kaizen/telemetry.ndjson under each workspace\n",
        );
    }
    block
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
pub fn cmd_telemetry_init(workspace: Option<&Path>, options: ConfigureOptions) -> Result<()> {
    cmd_telemetry_configure(workspace, options)
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
    if let Some(p) = provider_from_config(&cfg.telemetry) {
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
    let p = provider_from_config(&cfg.telemetry).ok_or_else(|| {
        anyhow::anyhow!(
            "no query provider resolved. Either:\n  \
             1. Run `kaizen telemetry configure --type=datadog` (or `posthog`) so the wizard \
             writes both `[[telemetry.exporters]]` and `[telemetry.query]`, OR\n  \
             2. Set `[telemetry.query].provider = \"datadog\"` in `~/.kaizen/config.toml` and \
             ensure DD_API_KEY is reachable (TOML row or env)."
        )
    })?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
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
    let home = kaizen_dir().ok_or_else(|| anyhow::anyhow!("KAIZEN_HOME / HOME unset"))?;
    let salt = effective_redaction_salt(&cfg.sync, &home).context(
        "resolve redaction salt (configured `[sync].team_salt_hex` or auto-generated `local_salt.hex`)",
    )?;
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
    let mut total_spans: u64 = 0;
    let mut total_batches: u64 = 0;
    let intake_warning_threshold_ms = end_ms.saturating_sub(18 * 3_600_000);
    let mut total_stale: u64 = 0;

    for root in &roots {
        let store = Store::open(&crate::core::workspace::db_path(root)?)?;
        let ws_key = root.to_string_lossy().to_string();
        let wh = workspace_hash(&salt, root.as_path());

        let event_rows = store.retro_events_in_window(&ws_key, start_ms, end_ms)?;
        let stale_events = event_rows
            .iter()
            .filter(|(_, ev)| ev.ts_ms < intake_warning_threshold_ms)
            .count() as u64;
        let outbound_events: Vec<_> = event_rows
            .into_iter()
            .map(|(session, ev)| {
                let mut o = outbound_event_from_row(&ev, &session, &salt);
                redact_payload(&mut o.payload, root.as_path(), &salt);
                o
            })
            .collect();
        let n_events = outbound_events.len() as u64;
        let event_batches = chunk_events_into_ingest_batches(
            team_id.clone(),
            wh.clone(),
            outbound_events,
            &cfg.sync,
        )?;

        let span_rows = store.tool_spans_sync_rows_in_window(&ws_key, start_ms, end_ms)?;
        let stale_spans = span_rows
            .iter()
            .filter(|r| {
                r.started_at_ms
                    .or(r.ended_at_ms)
                    .map(|t| t < intake_warning_threshold_ms)
                    .unwrap_or(false)
            })
            .count() as u64;
        let outbound_spans: Vec<_> = span_rows
            .iter()
            .map(|r| outbound_tool_span(r, &salt))
            .collect();
        let n_spans = outbound_spans.len() as u64;
        let span_batches =
            chunk_tool_spans_into_ingest_batches(team_id.clone(), wh, outbound_spans, &cfg.sync)?;

        let bcount = (event_batches.len() + span_batches.len()) as u64;
        total_events += n_events;
        total_spans += n_spans;
        total_batches += bcount;
        total_stale += stale_events + stale_spans;

        if dry_run {
            eprintln!(
                "telemetry push (dry-run): {} — {} event(s), {} span(s), {} batch(es)",
                root.display(),
                n_events,
                n_spans,
                bcount
            );
            continue;
        }
        for batch in event_batches.into_iter().chain(span_batches) {
            registry
                .fan_out(fail_open, &batch)
                .with_context(|| format!("telemetry fan-out ({})", batch.kind_name()))?;
        }
        eprintln!(
            "telemetry push: {} — sent {} event(s), {} span(s) in {} batch(es)",
            root.display(),
            n_events,
            n_spans,
            bcount
        );
    }

    eprintln!(
        "telemetry push: total {} event(s), {} span(s), {} batch(es) across {} workspace(s){}",
        total_events,
        total_spans,
        total_batches,
        roots.len(),
        if dry_run { " (dry-run)" } else { "" }
    );
    if total_stale > 0 {
        eprintln!(
            "note: {} item(s) have a `timestamp` older than 18h. Datadog Logs intake silently \
             drops these (organization default). PostHog/OTLP/file sinks accept them without \
             change. Use `--days N` with N <= 1 to skip stale items.",
            total_stale
        );
    }
    Ok(())
}

/// Send one synthetic redacted event through every configured exporter, report ok/fail per
/// sink. Pure observability: no SQLite read, no outbox enqueue, no Kaizen POST.
pub fn cmd_telemetry_test(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let registry = telemetry::load_exporters(&cfg.telemetry, ws.as_path());
    if registry.is_empty() {
        anyhow::bail!(
            "no `[[telemetry.exporters]]` rows resolved; run `kaizen telemetry configure --type=...` first"
        );
    }
    let batch = synthetic_batch(&cfg.sync.team_id);
    println!("telemetry test: sending one synthetic event to each configured sink ...");
    let mut all_ok = true;
    for name in registry.exporter_names() {
        match registry.export_one(&name, &batch) {
            Ok(()) => println!("  [{name}] ok"),
            Err(e) => {
                all_ok = false;
                println!("  [{name}] FAIL: {e:#}");
            }
        }
    }
    if !all_ok {
        anyhow::bail!("one or more exporters failed (see above)");
    }
    println!("telemetry test: all exporters accepted the synthetic event.");
    Ok(())
}

fn synthetic_batch(team_id: &str) -> IngestExportBatch {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    IngestExportBatch::Events(EventsBatchBody {
        team_id: team_id.to_string(),
        workspace_hash: "blake3:test-workspace".into(),
        events: vec![OutboundEvent {
            session_id_hash: "blake3:test-session".into(),
            event_seq: 0,
            ts_ms: now_ms,
            agent: "kaizen".into(),
            model: "synthetic".into(),
            kind: "lifecycle".into(),
            source: "tail".into(),
            tool: None,
            tool_call_id: None,
            tokens_in: Some(0),
            tokens_out: Some(0),
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: serde_json::json!({"kaizen.telemetry_test": true}),
        }],
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exporter_already_present_detects_existing_datadog_row() {
        let toml = r#"
[[telemetry.exporters]]
type = "datadog"
api_key = "abc"
site = "us5.datadoghq.com"
"#;
        assert!(exporter_already_present(toml, "datadog"));
        assert!(!exporter_already_present(toml, "posthog"));
    }

    #[test]
    fn exporter_already_present_handles_other_tables_between() {
        let toml = r#"
[[telemetry.exporters]]
type = "file"
enabled = true

[telemetry.query]
provider = "datadog"

[[telemetry.exporters]]
type = "datadog"
api_key = "abc"
"#;
        assert!(exporter_already_present(toml, "file"));
        assert!(exporter_already_present(toml, "datadog"));
        assert!(!exporter_already_present(toml, "otlp"));
    }

    #[test]
    fn reject_obvious_app_key_catches_ddapp_prefix() {
        assert!(reject_obvious_app_key("ddapp_FjBvwn3GKN8C6jiqltnbK0UHdUEs3gmlP1").is_some());
        assert!(reject_obvious_app_key("5bed85d67b7b0359bebeb40693537d0b").is_none());
    }

    #[test]
    fn ensure_query_authority_appends_when_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let p = dir.path().join("config.toml");
        std::fs::write(
            &p,
            "[[telemetry.exporters]]\ntype = \"datadog\"\napi_key = \"abc\"\n",
        )
        .unwrap();
        ensure_query_authority(&p, "datadog").unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains("[telemetry.query]"));
        assert!(s.contains("provider = \"datadog\""));
    }

    #[test]
    fn ensure_query_authority_idempotent_when_present() {
        let dir = tempfile::TempDir::new().unwrap();
        let p = dir.path().join("config.toml");
        let original = "[[telemetry.exporters]]\ntype = \"datadog\"\n\n[telemetry.query]\nprovider = \"posthog\"\n";
        std::fs::write(&p, original).unwrap();
        ensure_query_authority(&p, "datadog").unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        // User's existing posthog choice must NOT be overridden by the wizard.
        assert_eq!(s, original);
    }
}
