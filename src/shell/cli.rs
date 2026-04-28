// SPDX-License-Identifier: AGPL-3.0-or-later
//! CLI command implementations.

use crate::collect::tail::claude::scan_claude_session_dir;
use crate::collect::tail::codex::scan_codex_session_dir;
use crate::collect::tail::copilot_cli::scan_copilot_cli_workspace;
use crate::collect::tail::copilot_vscode::scan_copilot_vscode_workspace;
use crate::collect::tail::cursor::scan_session_dir_all;
use crate::collect::tail::goose::scan_goose_workspace;
use crate::collect::tail::openclaw::scan_openclaw_workspace;
use crate::collect::tail::opencode::scan_opencode_workspace;
use crate::core::config;
use crate::core::event::{Event, SessionRecord};
use crate::metrics::report;
use crate::shell::fmt::fmt_ts;
use crate::shell::scope;
use crate::store::{SYNC_STATE_LAST_AGENT_SCAN_MS, SYNC_STATE_LAST_AUTO_PRUNE_MS, Store};
use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

pub use crate::shell::init::cmd_init;
pub use crate::shell::insights::cmd_insights;

#[derive(Serialize)]
struct SessionsListJson {
    workspace: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    workspaces: Vec<String>,
    count: usize,
    sessions: Vec<SessionRecord>,
}

#[derive(Serialize)]
struct SummaryJsonOut {
    workspace: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    workspaces: Vec<String>,
    #[serde(flatten)]
    stats: crate::store::SummaryStats,
    cost_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    hotspot: Option<crate::metrics::types::RankedFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    slowest_tool: Option<crate::metrics::types::RankedTool>,
}

struct ScanSpinner(Option<indicatif::ProgressBar>);

impl ScanSpinner {
    fn start(msg: &'static str) -> Self {
        if !std::io::stdout().is_terminal() {
            return Self(None);
        }
        let p = indicatif::ProgressBar::new_spinner();
        p.set_message(msg.to_string());
        p.enable_steady_tick(std::time::Duration::from_millis(120));
        Self(Some(p))
    }
}

impl Drop for ScanSpinner {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            p.finish_and_clear();
        }
    }
}

fn now_ms_u64() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Minimum interval between automatic local DB prunes after a successful rescan (24h).
const AUTO_PRUNE_INTERVAL_MS: u64 = 86_400_000;

pub(crate) fn maybe_auto_prune_after_scan(store: &Store, cfg: &config::Config) -> Result<()> {
    if cfg.retention.hot_days == 0 {
        return Ok(());
    }
    let now = now_ms_u64();
    if let Some(last) = store.sync_state_get_u64(SYNC_STATE_LAST_AUTO_PRUNE_MS)?
        && now.saturating_sub(last) < AUTO_PRUNE_INTERVAL_MS
    {
        return Ok(());
    }
    let cutoff = now.saturating_sub((cfg.retention.hot_days as u64).saturating_mul(86_400_000));
    store.prune_sessions_started_before(cutoff as i64)?;
    store.sync_state_set_u64(SYNC_STATE_LAST_AUTO_PRUNE_MS, now)?;
    Ok(())
}

/// Full transcript rescan unless throttled by `[scan].min_rescan_seconds` or `refresh` is true.
pub(crate) fn maybe_scan_all_agents(
    ws: &Path,
    cfg: &config::Config,
    ws_str: &str,
    store: &Store,
    refresh: bool,
) -> Result<()> {
    let interval_ms = cfg.scan.min_rescan_seconds.saturating_mul(1000);
    let now = now_ms_u64();
    if !refresh
        && interval_ms > 0
        && let Some(last) = store.sync_state_get_u64(SYNC_STATE_LAST_AGENT_SCAN_MS)?
        && now.saturating_sub(last) < interval_ms
    {
        return Ok(());
    }
    scan_all_agents(ws, cfg, ws_str, store)?;
    store.sync_state_set_u64(SYNC_STATE_LAST_AGENT_SCAN_MS, now_ms_u64())?;
    Ok(())
}

pub(crate) fn maybe_refresh_store(workspace: &Path, store: &Store, refresh: bool) -> Result<()> {
    if !refresh {
        return Ok(());
    }
    let cfg = config::load(workspace)?;
    let ws_str = workspace.to_string_lossy().to_string();
    maybe_scan_all_agents(workspace, &cfg, &ws_str, store, true)
}

fn combine_counts(rows: Vec<Vec<(String, u64)>>) -> Vec<(String, u64)> {
    let mut counts = HashMap::new();
    for set in rows {
        for (key, value) in set {
            *counts.entry(key).or_insert(0_u64) += value;
        }
    }
    let mut out = counts.into_iter().collect::<Vec<_>>();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

fn workspace_names(roots: &[PathBuf]) -> Vec<String> {
    roots
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

fn open_workspace_store(workspace: &Path) -> Result<Store> {
    Store::open(&crate::core::workspace::db_path(workspace))
}

/// `kaizen sessions list` — same output as CLI stdout.
pub fn sessions_list_text(
    workspace: Option<&Path>,
    json_out: bool,
    refresh: bool,
    all_workspaces: bool,
) -> Result<String> {
    let roots = scope::resolve(workspace, all_workspaces)?;
    let mut sessions = Vec::new();
    if crate::daemon::enabled() && !refresh {
        for workspace in &roots {
            let ws_str = workspace.to_string_lossy().to_string();
            let response =
                crate::daemon::request_blocking(crate::ipc::DaemonRequest::ListSessions {
                    workspace: ws_str,
                    offset: 0,
                    limit: i64::MAX as usize,
                    filter: crate::store::SessionFilter::default(),
                })?;
            match response {
                crate::ipc::DaemonResponse::Sessions(page) => sessions.extend(page.rows),
                crate::ipc::DaemonResponse::Error { message, .. } => anyhow::bail!(message),
                _ => anyhow::bail!("unexpected daemon sessions response"),
            }
        }
    } else {
        for workspace in &roots {
            let store = open_workspace_store(workspace)?;
            maybe_refresh_store(workspace, &store, refresh)?;
            let ws_str = workspace.to_string_lossy().to_string();
            sessions.extend(store.list_sessions(&ws_str)?);
        }
    }
    sessions.sort_by(|a, b| {
        b.started_at_ms
            .cmp(&a.started_at_ms)
            .then_with(|| a.id.cmp(&b.id))
    });
    let scope_label = scope::label(&roots);
    let workspaces = if roots.len() > 1 {
        workspace_names(&roots)
    } else {
        Vec::new()
    };
    if json_out {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&SessionsListJson {
                workspace: scope_label,
                workspaces,
                count: sessions.len(),
                sessions,
            })?
        ));
    }
    use std::fmt::Write;
    let mut out = String::new();
    if roots.len() > 1 {
        writeln!(&mut out, "Scope: {scope_label}").unwrap();
        writeln!(&mut out).unwrap();
    }
    writeln!(
        &mut out,
        "{:<40} {:<10} {:<10} STARTED",
        "ID", "AGENT", "STATUS"
    )
    .unwrap();
    writeln!(&mut out, "{}", "-".repeat(80)).unwrap();
    for s in &sessions {
        writeln!(
            &mut out,
            "{:<40} {:<10} {:<10} {}",
            s.id,
            s.agent,
            format!("{:?}", s.status),
            fmt_ts(s.started_at_ms),
        )
        .unwrap();
    }
    if sessions.is_empty() {
        writeln!(&mut out, "(no sessions)").unwrap();
        sessions_empty_state_hints(&mut out);
    }
    Ok(out)
}

fn sessions_empty_state_hints(out: &mut String) {
    use std::fmt::Write;
    let _ = writeln!(out);
    let _ = writeln!(out, "No sessions found for this workspace. Try:");
    let _ = writeln!(out, "  · `kaizen doctor` — verify config and hooks");
    let _ = writeln!(out, "  · a short agent session in this repo, then re-run");
    let _ = writeln!(
        out,
        "  · docs: https://github.com/marquesds/kaizen/blob/main/docs/config.md (sources)"
    );
}

/// `kaizen sessions list` — scan all agent transcripts, upsert sessions, print table.
pub fn cmd_sessions_list(
    workspace: Option<&Path>,
    json_out: bool,
    refresh: bool,
    all_workspaces: bool,
) -> Result<()> {
    print!(
        "{}",
        sessions_list_text(workspace, json_out, refresh, all_workspaces)?
    );
    Ok(())
}

/// `kaizen sessions show` — same output as CLI stdout.
pub fn session_show_text(id: &str, workspace: Option<&Path>) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = open_workspace_store(&ws)?;
    use std::fmt::Write;
    let mut out = String::new();
    match store.get_session(id)? {
        Some(s) => {
            writeln!(&mut out, "id:           {}", s.id).unwrap();
            writeln!(&mut out, "agent:        {}", s.agent).unwrap();
            writeln!(
                &mut out,
                "model:        {}",
                s.model.as_deref().unwrap_or("-")
            )
            .unwrap();
            writeln!(&mut out, "workspace:    {}", s.workspace).unwrap();
            writeln!(&mut out, "started_at:   {}", fmt_ts(s.started_at_ms)).unwrap();
            writeln!(
                &mut out,
                "ended_at:     {}",
                s.ended_at_ms.map(fmt_ts).unwrap_or_else(|| "-".to_string())
            )
            .unwrap();
            writeln!(&mut out, "status:       {:?}", s.status).unwrap();
            writeln!(&mut out, "trace_path:   {}", s.trace_path).unwrap();
            if let Some(fp) = &s.prompt_fingerprint {
                writeln!(&mut out, "prompt_fp:    {fp}").unwrap();
                if let Ok(Some(snap)) = store.get_prompt_snapshot(fp) {
                    for f in snap.files() {
                        writeln!(&mut out, "  - {}", f.path).unwrap();
                    }
                }
            }
        }
        None => anyhow::bail!("session not found: {id} — try `kaizen sessions list`"),
    }
    let evals = store.list_evals_for_session(id).unwrap_or_default();
    if !evals.is_empty() {
        writeln!(&mut out, "evals:").unwrap();
        for e in &evals {
            writeln!(
                &mut out,
                "  {} score={:.2} flagged={} {}",
                e.rubric_id, e.score, e.flagged, e.rationale
            )
            .unwrap();
        }
    }
    let fb = store
        .feedback_for_sessions(&[id.to_string()])
        .unwrap_or_default();
    if let Some(r) = fb.get(id) {
        let score = r
            .score
            .as_ref()
            .map(|s| s.0.to_string())
            .unwrap_or_else(|| "-".into());
        let label = r
            .label
            .as_ref()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "-".into());
        writeln!(&mut out, "feedback:     score={score} label={label}").unwrap();
        if let Some(n) = &r.note {
            writeln!(&mut out, "  note: {n}").unwrap();
        }
    }
    Ok(out)
}

/// `kaizen sessions show <id>` — print full session fields.
pub fn cmd_session_show(id: &str, workspace: Option<&Path>) -> Result<()> {
    print!("{}", session_show_text(id, workspace)?);
    Ok(())
}

pub fn sessions_tree_text(id: &str, max_depth: u32, workspace: Option<&Path>) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = open_workspace_store(&ws)?;
    let nodes = store.session_span_tree(id)?;
    let total_cost: i64 = nodes.iter().map(|n| n.subtree_cost_usd_e6).sum();
    let mut out = String::new();
    for node in &nodes {
        render_node(&mut out, node, 0, max_depth, total_cost);
    }
    Ok(out)
}

fn render_node(
    out: &mut String,
    node: &crate::store::span_tree::SpanNode,
    depth: u32,
    max_depth: u32,
    session_total: i64,
) {
    use std::fmt::Write;
    if depth > max_depth {
        return;
    }
    let indent = "│  ".repeat(depth as usize);
    let prefix = if depth == 0 { "┌─ " } else { "├─ " };
    let cost_str = match node.span.subtree_cost_usd_e6 {
        Some(c) => {
            let pct = if session_total > 0 {
                c * 100 / session_total
            } else {
                0
            };
            let flag = if pct > 40 { " ⚡" } else { "" };
            format!(" ${:.4}{}", c as f64 / 1_000_000.0, flag)
        }
        None => String::new(),
    };
    writeln!(
        out,
        "{}{}{} [{}]{}",
        indent, prefix, node.span.tool, node.span.status, cost_str
    )
    .unwrap();
    for child in &node.children {
        render_node(out, child, depth + 1, max_depth, session_total);
    }
}

/// `kaizen sessions tree <id>` — produce text output (ASCII or JSON).
pub fn cmd_sessions_tree_text(
    id: &str,
    depth: u32,
    json: bool,
    workspace: Option<&Path>,
) -> Result<String> {
    if json {
        let ws = workspace_path(workspace)?;
        let store = open_workspace_store(&ws)?;
        let nodes = store.session_span_tree(id)?;
        Ok(serde_json::to_string_pretty(&nodes)?)
    } else {
        sessions_tree_text(id, depth, workspace)
    }
}

/// `kaizen sessions tree <id>` — print ASCII span tree.
pub fn cmd_sessions_tree(id: &str, depth: u32, json: bool, workspace: Option<&Path>) -> Result<()> {
    print!("{}", cmd_sessions_tree_text(id, depth, json, workspace)?);
    Ok(())
}

/// `kaizen summary` — same output as CLI stdout.
pub fn summary_text(
    workspace: Option<&Path>,
    json_out: bool,
    refresh: bool,
    all_workspaces: bool,
    source: crate::core::data_source::DataSource,
) -> Result<String> {
    let roots = scope::resolve(workspace, all_workspaces)?;
    let mut total_cost_usd_e6 = 0_i64;
    let mut session_count = 0_u64;
    let mut by_agent = Vec::new();
    let mut by_model = Vec::new();
    let mut top_tools = Vec::new();
    let mut hottest = Vec::new();
    let mut slowest = Vec::new();

    for workspace in &roots {
        let cfg = config::load(workspace)?;
        let store = open_workspace_store(workspace)?;
        crate::shell::remote_pull::maybe_telemetry_pull(workspace, &store, &cfg, source, refresh)?;
        maybe_refresh_store(workspace, &store, refresh)?;
        let ws_str = workspace.to_string_lossy().to_string();
        let read_store = Store::open_read_only(&crate::core::workspace::db_path(workspace))?;
        let mut stats = read_store.summary_stats(&ws_str)?;
        if source != crate::core::data_source::DataSource::Local
            && let Ok(Some(agg)) =
                crate::shell::remote_observe::try_remote_event_agg(&read_store, &cfg, workspace)
        {
            stats = crate::shell::remote_observe::merge_summary_stats(stats, &agg, source);
        }
        total_cost_usd_e6 += stats.total_cost_usd_e6;
        session_count += stats.session_count;
        by_agent.push(stats.by_agent);
        by_model.push(stats.by_model);
        top_tools.push(stats.top_tools);
        if let Ok(metrics) = report::build_report(&read_store, &ws_str, 7) {
            if let Some(file) = metrics.hottest_files.first().cloned() {
                hottest.push(if roots.len() == 1 {
                    file
                } else {
                    crate::metrics::types::RankedFile {
                        path: scope::decorate_path(workspace, &file.path),
                        ..file
                    }
                });
            }
            if let Some(tool) = metrics.slowest_tools.first().cloned() {
                slowest.push(tool);
            }
        }
    }

    let stats = crate::store::SummaryStats {
        session_count,
        total_cost_usd_e6,
        by_agent: combine_counts(by_agent),
        by_model: combine_counts(by_model),
        top_tools: combine_counts(top_tools),
    };
    let cost_dollars = stats.total_cost_usd_e6 as f64 / 1_000_000.0;
    let hotspot = hottest
        .into_iter()
        .max_by(|a, b| a.value.cmp(&b.value).then_with(|| b.path.cmp(&a.path)));
    let slowest_tool = slowest.into_iter().max_by(|a, b| {
        a.p95_ms
            .unwrap_or(0)
            .cmp(&b.p95_ms.unwrap_or(0))
            .then_with(|| b.tool.cmp(&a.tool))
    });
    let scope_label = scope::label(&roots);
    let workspaces = if roots.len() > 1 {
        workspace_names(&roots)
    } else {
        Vec::new()
    };
    if json_out {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&SummaryJsonOut {
                workspace: scope_label,
                workspaces,
                cost_usd: cost_dollars,
                stats,
                hotspot,
                slowest_tool,
            })?
        ));
    }
    use std::fmt::Write;
    let mut out = String::new();
    if roots.len() > 1 {
        writeln!(&mut out, "Scope: {}", scope::label(&roots)).unwrap();
    }
    writeln!(
        &mut out,
        "Sessions: {}   Cost: ${:.2}",
        stats.session_count, cost_dollars
    )
    .unwrap();

    if !stats.by_agent.is_empty() {
        let parts: Vec<String> = stats
            .by_agent
            .iter()
            .map(|(a, n)| format!("{a} {n}"))
            .collect();
        writeln!(&mut out, "By agent:  {}", parts.join(" · ")).unwrap();
    }
    if !stats.by_model.is_empty() {
        let parts: Vec<String> = stats
            .by_model
            .iter()
            .map(|(m, n)| format!("{m} {n}"))
            .collect();
        writeln!(&mut out, "By model:  {}", parts.join(" · ")).unwrap();
    }
    if !stats.top_tools.is_empty() {
        let parts: Vec<String> = stats
            .top_tools
            .iter()
            .take(5)
            .map(|(t, n)| format!("{t} {n}"))
            .collect();
        writeln!(&mut out, "Top tools: {}", parts.join(" · ")).unwrap();
    }
    if let Some(file) = hotspot {
        writeln!(&mut out, "Hotspot:   {} ({})", file.path, file.value).unwrap();
    }
    if let Some(tool) = slowest_tool {
        let p95 = tool
            .p95_ms
            .map(|v| format!("{v}ms"))
            .unwrap_or_else(|| "-".into());
        writeln!(&mut out, "Slowest:   {} p95 {}", tool.tool, p95).unwrap();
    }
    Ok(out)
}

/// `kaizen summary` — aggregate session + cost stats across all agents.
pub fn cmd_summary(
    workspace: Option<&Path>,
    json_out: bool,
    refresh: bool,
    all_workspaces: bool,
    source: crate::core::data_source::DataSource,
) -> Result<()> {
    print!(
        "{}",
        summary_text(workspace, json_out, refresh, all_workspaces, source,)?
    );
    Ok(())
}

pub(crate) fn scan_all_agents(
    ws: &Path,
    cfg: &config::Config,
    ws_str: &str,
    store: &Store,
) -> Result<()> {
    let _spin = ScanSpinner::start("Scanning agent sessions…");
    let slug = workspace_slug(ws_str);
    let sync_ctx = crate::sync::ingest_ctx(cfg, ws.to_path_buf());

    for root in &cfg.scan.roots {
        let expanded = expand_home(root);
        let cursor_dir = PathBuf::from(&expanded)
            .join(&slug)
            .join("agent-transcripts");
        scan_agent_dirs(
            &cursor_dir,
            store,
            |p| {
                scan_session_dir_all(p).map(|sessions| {
                    sessions
                        .into_iter()
                        .map(|(mut r, evs)| {
                            r.workspace = ws_str.to_string();
                            (r, evs)
                        })
                        .collect()
                })
            },
            sync_ctx.as_ref(),
        )?;
    }

    let home = std::env::var("HOME").unwrap_or_default();

    let claude_dir = PathBuf::from(&home)
        .join(".claude/projects")
        .join(&slug)
        .join("sessions");
    scan_agent_dirs(
        &claude_dir,
        store,
        |p| {
            scan_claude_session_dir(p).map(|(mut r, evs)| {
                r.workspace = ws_str.to_string();
                vec![(r, evs)]
            })
        },
        sync_ctx.as_ref(),
    )?;

    let codex_dir = PathBuf::from(&home).join(".codex/sessions").join(&slug);
    scan_agent_dirs(
        &codex_dir,
        store,
        |p| {
            scan_codex_session_dir(p).map(|(mut r, evs)| {
                r.workspace = ws_str.to_string();
                vec![(r, evs)]
            })
        },
        sync_ctx.as_ref(),
    )?;

    let tail = &cfg.sources.tail;
    let home_pb = PathBuf::from(&home);
    if tail.goose {
        let sessions = scan_goose_workspace(&home_pb, ws)?;
        persist_session_batch(store, sessions, sync_ctx.as_ref())?;
    }
    if tail.openclaw {
        let sessions = scan_openclaw_workspace(ws)?;
        persist_session_batch(store, sessions, sync_ctx.as_ref())?;
    }
    if tail.opencode {
        let sessions = scan_opencode_workspace(ws)?;
        persist_session_batch(store, sessions, sync_ctx.as_ref())?;
    }
    if tail.copilot_cli {
        let sessions = scan_copilot_cli_workspace(ws)?;
        persist_session_batch(store, sessions, sync_ctx.as_ref())?;
    }
    if tail.copilot_vscode {
        let sessions = scan_copilot_vscode_workspace(ws)?;
        persist_session_batch(store, sessions, sync_ctx.as_ref())?;
    }

    maybe_auto_prune_after_scan(store, cfg)?;
    Ok(())
}

fn persist_session_batch(
    store: &Store,
    sessions: Vec<(SessionRecord, Vec<Event>)>,
    sync_ctx: Option<&crate::sync::SyncIngestContext>,
) -> Result<()> {
    for (mut record, events) in sessions {
        if record.start_commit.is_none() && !record.workspace.is_empty() {
            let binding = crate::core::repo::binding_for_session(
                Path::new(&record.workspace),
                record.started_at_ms,
                record.ended_at_ms,
            );
            record.start_commit = binding.start_commit;
            record.end_commit = binding.end_commit;
            record.branch = binding.branch;
            record.dirty_start = binding.dirty_start;
            record.dirty_end = binding.dirty_end;
            record.repo_binding_source = binding.source;
        }
        store.upsert_session(&record)?;
        let flush_ms = record.ended_at_ms.unwrap_or(record.started_at_ms);
        for ev in events {
            store.append_event_with_sync(&ev, sync_ctx)?;
        }
        if record.status == crate::core::event::SessionStatus::Done {
            store.flush_projector_session(&record.id, flush_ms)?;
        }
    }
    Ok(())
}

pub(crate) fn scan_agent_dirs<F>(
    dir: &Path,
    store: &Store,
    scanner: F,
    sync_ctx: Option<&crate::sync::SyncIngestContext>,
) -> Result<()>
where
    F: Fn(&Path) -> Result<Vec<(SessionRecord, Vec<Event>)>>,
{
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        match scanner(&entry.path()) {
            Ok(sessions) => {
                for (mut record, events) in sessions {
                    if record.start_commit.is_none() && !record.workspace.is_empty() {
                        let binding = crate::core::repo::binding_for_session(
                            Path::new(&record.workspace),
                            record.started_at_ms,
                            record.ended_at_ms,
                        );
                        record.start_commit = binding.start_commit;
                        record.end_commit = binding.end_commit;
                        record.branch = binding.branch;
                        record.dirty_start = binding.dirty_start;
                        record.dirty_end = binding.dirty_end;
                        record.repo_binding_source = binding.source;
                    }
                    store.upsert_session(&record)?;
                    let flush_ms = record.ended_at_ms.unwrap_or(record.started_at_ms);
                    for ev in events {
                        store.append_event_with_sync(&ev, sync_ctx)?;
                    }
                    if record.status == crate::core::event::SessionStatus::Done {
                        store.flush_projector_session(&record.id, flush_ms)?;
                    }
                }
            }
            Err(e) => tracing::warn!("scan {:?}: {e}", entry.path()),
        }
    }
    Ok(())
}

pub(crate) fn workspace_path(workspace: Option<&Path>) -> Result<PathBuf> {
    crate::core::workspace::resolve(workspace)
}

/// Convert workspace path to cursor project slug.
/// `/Users/lucas/Projects/kaizen` → `Users-lucas-Projects-kaizen`
pub(crate) fn workspace_slug(ws: &str) -> String {
    ws.trim_start_matches('/').replace('/', "-")
}

pub(crate) fn expand_home(path: &str) -> String {
    if let (Some(rest), Ok(home)) = (path.strip_prefix("~/"), std::env::var("HOME")) {
        return format!("{home}/{rest}");
    }
    path.to_string()
}
