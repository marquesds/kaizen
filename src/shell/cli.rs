//! CLI command implementations.

use crate::collect::tail::claude::scan_claude_session_dir;
use crate::collect::tail::codex::scan_codex_session_dir;
use crate::collect::tail::cursor::scan_session_dir_all;
use crate::core::config;
use crate::core::event::{Event, SessionRecord};
use crate::metrics::{index, report};
use crate::shell::fmt::fmt_ts;
use crate::store::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub use crate::shell::init::cmd_init;
pub use crate::shell::insights::cmd_insights;

/// `kaizen sessions list` — scan all agent transcripts, upsert sessions, print table.
pub fn cmd_sessions_list(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let ws_str = ws.to_string_lossy().to_string();

    scan_all_agents(&ws, &cfg, &ws_str, &store)?;

    let sessions = store.list_sessions(&ws_str)?;
    println!("{:<40} {:<10} {:<10} STARTED", "ID", "AGENT", "STATUS");
    println!("{}", "-".repeat(80));
    for s in &sessions {
        println!(
            "{:<40} {:<10} {:<10} {}",
            s.id,
            s.agent,
            format!("{:?}", s.status),
            fmt_ts(s.started_at_ms),
        );
    }
    if sessions.is_empty() {
        println!("(no sessions)");
    }
    Ok(())
}

/// `kaizen sessions show <id>` — print full session fields.
pub fn cmd_session_show(id: &str, workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    match store.get_session(id)? {
        Some(s) => {
            println!("id:           {}", s.id);
            println!("agent:        {}", s.agent);
            println!("model:        {}", s.model.as_deref().unwrap_or("-"));
            println!("workspace:    {}", s.workspace);
            println!("started_at:   {}", fmt_ts(s.started_at_ms));
            println!(
                "ended_at:     {}",
                s.ended_at_ms.map(fmt_ts).unwrap_or_else(|| "-".to_string())
            );
            println!("status:       {:?}", s.status);
            println!("trace_path:   {}", s.trace_path);
        }
        None => anyhow::bail!("session not found: {id} — try `kaizen sessions list`"),
    }
    Ok(())
}

/// `kaizen summary` — aggregate session + cost stats across all agents.
pub fn cmd_summary(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let ws_str = ws.to_string_lossy().to_string();

    scan_all_agents(&ws, &cfg, &ws_str, &store)?;

    let stats = store.summary_stats(&ws_str)?;
    let cost_dollars = stats.total_cost_usd_e6 as f64 / 1_000_000.0;
    println!(
        "Sessions: {}   Cost: ${:.2}",
        stats.session_count, cost_dollars
    );

    if !stats.by_agent.is_empty() {
        let parts: Vec<String> = stats
            .by_agent
            .iter()
            .map(|(a, n)| format!("{a} {n}"))
            .collect();
        println!("By agent:  {}", parts.join(" · "));
    }
    if !stats.by_model.is_empty() {
        let parts: Vec<String> = stats
            .by_model
            .iter()
            .map(|(m, n)| format!("{m} {n}"))
            .collect();
        println!("By model:  {}", parts.join(" · "));
    }
    if !stats.top_tools.is_empty() {
        let parts: Vec<String> = stats
            .top_tools
            .iter()
            .map(|(t, n)| format!("{t} {n}"))
            .collect();
        println!("Top tools: {}", parts.join(" · "));
    }
    if let Ok(_snapshot) = index::ensure_indexed(&store, &ws, false)
        && let Ok(metrics) = report::build_report(&store, &ws_str, 7)
    {
        if let Some(ctx) = crate::sync::ingest_ctx(&cfg, ws.clone())
            && let Some(snapshot) = metrics.snapshot.as_ref()
            && let Ok(facts) = store.file_facts_for_snapshot(&snapshot.id)
            && let Ok(edges) = store.repo_edges_for_snapshot(&snapshot.id)
        {
            let _ =
                crate::sync::smart::enqueue_repo_snapshot(&store, snapshot, &facts, &edges, &ctx);
        }
        if let Some(file) = metrics.hottest_files.first() {
            println!("Hotspot:   {} ({})", file.path, file.value);
        }
        if let Some(tool) = metrics.slowest_tools.first() {
            let p95 = tool
                .p95_ms
                .map(|v| format!("{v}ms"))
                .unwrap_or_else(|| "-".into());
            println!("Slowest:   {} p95 {}", tool.tool, p95);
        }
    }
    Ok(())
}

pub(crate) fn scan_all_agents(
    ws: &Path,
    cfg: &config::Config,
    ws_str: &str,
    store: &Store,
) -> Result<()> {
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
                    for ev in events {
                        store.append_event_with_sync(&ev, sync_ctx)?;
                    }
                }
            }
            Err(e) => tracing::warn!("scan {:?}: {e}", entry.path()),
        }
    }
    Ok(())
}

pub(crate) fn workspace_path(workspace: Option<&Path>) -> Result<PathBuf> {
    match workspace {
        Some(p) => Ok(p.to_path_buf()),
        None => std::env::current_dir().map_err(Into::into),
    }
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
