// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen sessions search` and `kaizen search reindex`.

use crate::core::config::{self, try_team_salt};
use crate::core::event::Event;
use crate::search::extract::{redacted_event_text, snippet};
use crate::search::{SearchHit, SearchQuery};
use crate::shell::cli::{open_workspace_read_store, workspace_path};
use crate::shell::fmt::fmt_ts;
use crate::store::Store;
use crate::store::event_index::{is_valid_slug, paths_from_event_payload, skills_from_event_json};
use anyhow::{Context, Result};
use std::path::Path;

pub fn cmd_sessions_search(
    workspace: Option<&Path>,
    query: &str,
    since: Option<&str>,
    agent: Option<&str>,
    kind: Option<&str>,
    limit: usize,
) -> Result<()> {
    print!(
        "{}",
        sessions_search_text(workspace, query, since, agent, kind, limit)?
    );
    Ok(())
}

pub fn sessions_search_text(
    workspace: Option<&Path>,
    query: &str,
    since: Option<&str>,
    agent: Option<&str>,
    kind: Option<&str>,
    limit: usize,
) -> Result<String> {
    let (hits, fallback) = sessions_search_hits(workspace, query, since, agent, kind, limit)?;
    render_hits(&hits, fallback)
}

pub fn sessions_search_hits(
    workspace: Option<&Path>,
    query: &str,
    since: Option<&str>,
    agent: Option<&str>,
    kind: Option<&str>,
    limit: usize,
) -> Result<(Vec<SearchHit>, bool)> {
    let ws = workspace_path(workspace)?;
    let store = open_workspace_read_store(&ws, false)?;
    let cfg = config::load(&ws)?;
    let salt = try_team_salt(&cfg.sync).unwrap_or([0; 32]);
    let opts = SearchQuery {
        query: query.to_string(),
        since_ms: parse_since(since)?,
        agent: agent.map(str::to_string),
        kind: kind.map(str::to_string),
        limit,
    };
    let data_dir = crate::core::paths::project_data_dir(&ws)?;
    if let Some(hits) = exact_tool_hits(&store, &ws, &opts, &salt)? {
        return Ok((hits, false));
    }
    match crate::search::search(&data_dir, &opts, &ws, &salt, |s, q| store.get_event(s, q)) {
        Ok(hits) => Ok((hits, false)),
        Err(e) => anyhow::bail!("search index unavailable: {e}; run `kaizen search reindex`"),
    }
}

fn exact_tool_hits(
    store: &Store,
    workspace: &Path,
    opts: &SearchQuery,
    salt: &[u8; 32],
) -> Result<Option<Vec<SearchHit>>> {
    let Some(tool) = exact_tool_query(opts) else {
        return Ok(None);
    };
    let ws = workspace.to_string_lossy();
    let rows =
        store.search_tool_events(&ws, tool, opts.since_ms, opts.agent.as_deref(), opts.limit)?;
    let hits = rows
        .into_iter()
        .map(|(agent, event)| tool_hit(agent, event, workspace, salt, &opts.query))
        .collect::<Vec<_>>();
    Ok((!hits.is_empty()).then_some(hits))
}

fn exact_tool_query(opts: &SearchQuery) -> Option<&str> {
    if opts.limit == 0 || opts.kind.is_some() || !is_valid_slug(&opts.query) {
        return None;
    }
    Some(opts.query.as_str())
}

fn tool_hit(
    agent: String,
    event: Event,
    workspace: &Path,
    salt: &[u8; 32],
    query: &str,
) -> SearchHit {
    let text = redacted_event_text(&event, workspace, salt);
    SearchHit {
        session_id: event.session_id.clone(),
        seq: event.seq,
        ts_ms: event.ts_ms,
        agent,
        kind: crate::search::kind_label(&event.kind)
            .unwrap_or("unknown")
            .to_string(),
        score: 1.0,
        snippet: snippet(&text, query),
        paths: paths_from_event_payload(&event.payload),
        skills: skills_from_event_json(&event.payload),
        tokens_total: crate::search::tokens_total(&event),
    }
}

pub fn cmd_search_reindex(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let cfg = config::load(&ws)?;
    let ws_str = ws.to_string_lossy().to_string();
    let sessions = store.list_sessions(&ws_str)?;
    let events = store.workspace_events(&ws_str)?;
    let data_dir = crate::core::paths::project_data_dir(&ws)?;
    let stats = crate::search::reindex_workspace(&data_dir, &ws, &sessions, events, &cfg)
        .context("reindex search")?;
    println!(
        "search reindex: {} events seen, {} docs indexed",
        stats.events_seen, stats.docs_indexed
    );
    Ok(())
}

fn render_hits(hits: &[SearchHit], fallback: bool) -> Result<String> {
    use std::fmt::Write;
    let mut out = String::new();
    if fallback {
        writeln!(
            out,
            "warning: search index unavailable; falling back to event scan"
        )?;
    }
    writeln!(out, "{:<40} {:<19} {:>7} SNIPPET", "SESSION", "TS", "SCORE")?;
    for h in hits {
        writeln!(
            out,
            "{:<40} {:<19} {:>7.3} {}",
            h.session_id,
            fmt_ts(h.ts_ms),
            h.score,
            h.snippet
        )?;
    }
    Ok(out)
}

fn parse_since(raw: Option<&str>) -> Result<Option<u64>> {
    let Some(raw) = raw else { return Ok(None) };
    let days = raw.trim_end_matches('d').parse::<u64>()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    Ok(Some(
        (now.as_millis() as u64).saturating_sub(days.saturating_mul(86_400_000)),
    ))
}
