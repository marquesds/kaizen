// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen sessions search` and `kaizen search reindex`.

use crate::core::config::{self, try_team_salt};
use crate::search::{SearchHit, SearchQuery};
use crate::shell::cli::workspace_path;
use crate::shell::fmt::fmt_ts;
use crate::store::Store;
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
    let store = Store::open(&crate::core::workspace::db_path(&ws))?;
    store.flush_search().ok();
    let cfg = config::load(&ws)?;
    let salt = try_team_salt(&cfg.sync).unwrap_or([0; 32]);
    let opts = SearchQuery {
        query: query.to_string(),
        since_ms: parse_since(since)?,
        agent: agent.map(str::to_string),
        kind: kind.map(str::to_string),
        limit,
    };
    match crate::search::search(&ws.join(".kaizen"), &opts, &ws, &salt, |s, q| {
        store.get_event(s, q)
    }) {
        Ok(hits) => Ok((hits, false)),
        Err(_) => Ok((scan_fallback(&store, &ws, &opts, &salt)?, true)),
    }
}

pub fn cmd_search_reindex(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws))?;
    let cfg = config::load(&ws)?;
    let ws_str = ws.to_string_lossy().to_string();
    let sessions = store.list_sessions(&ws_str)?;
    let events = store.workspace_events(&ws_str)?;
    let stats = crate::search::reindex_workspace(&ws.join(".kaizen"), &ws, &sessions, events, &cfg)
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

fn scan_fallback(
    store: &Store,
    ws: &Path,
    opts: &SearchQuery,
    salt: &[u8; 32],
) -> Result<Vec<SearchHit>> {
    let mut out = Vec::new();
    for (session, event) in store.workspace_events(&ws.to_string_lossy())? {
        if out.len() >= opts.limit {
            break;
        }
        let Some(doc) = crate::search::extract_doc(&event, &session, ws, salt) else {
            continue;
        };
        if !scan_match(&doc, opts) {
            continue;
        }
        out.push(SearchHit {
            session_id: doc.session_id,
            seq: doc.seq,
            ts_ms: doc.ts_ms,
            agent: doc.agent,
            kind: doc.kind,
            score: 0.0,
            snippet: crate::search::extract::snippet(&doc.text, &opts.query),
            paths: doc.paths,
            skills: doc.skills,
            tokens_total: doc.tokens_total,
        });
    }
    Ok(out)
}

fn scan_match(doc: &crate::search::SearchDoc, opts: &SearchQuery) -> bool {
    let q = opts.query.trim_matches('"').to_lowercase();
    opts.agent.as_ref().is_none_or(|a| &doc.agent == a)
        && opts.kind.as_ref().is_none_or(|k| &doc.kind == k)
        && opts.since_ms.is_none_or(|ms| doc.ts_ms >= ms)
        && (doc.text.to_lowercase().contains(&q)
            || doc.paths.iter().any(|p| opts.query.contains(p)))
}

fn parse_since(raw: Option<&str>) -> Result<Option<u64>> {
    let Some(raw) = raw else { return Ok(None) };
    let days = raw.trim_end_matches('d').parse::<u64>()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?;
    Ok(Some(
        (now.as_millis() as u64).saturating_sub(days.saturating_mul(86_400_000)),
    ))
}
