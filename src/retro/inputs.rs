// SPDX-License-Identifier: AGPL-3.0-or-later
//! Build [`Inputs`] from SQLite + workspace filesystem.

use crate::core::data_source::DataSource;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::retro::types::{Inputs, RetroAggregates, SkillFileOnDisk};
use crate::store::Store;
use crate::sync::outbound::OutboundEvent;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

const USAGE_LOOKBACK_MIN_DAYS: u64 = 30;

/// Load retro inputs after the store has been refreshed (e.g. `scan_all_agents`).
pub fn load_inputs(
    store: &Store,
    workspace_root: &Path,
    workspace_key: &str,
    window_start_ms: u64,
    window_end_ms: u64,
) -> Result<Inputs> {
    let events = store.retro_events_in_window(workspace_key, window_start_ms, window_end_ms)?;
    let files_touched =
        store.files_touched_in_window(workspace_key, window_start_ms, window_end_ms)?;
    let skills_used = store.skills_used_in_window(workspace_key, window_start_ms, window_end_ms)?;
    let tool_spans = store.tool_spans_in_window(workspace_key, window_start_ms, window_end_ms)?;

    let lookback_start = window_end_ms.saturating_sub(USAGE_LOOKBACK_MIN_DAYS * 86_400_000);
    let recent_slugs_list = store.skills_used_since(workspace_key, lookback_start)?;
    let skills_used_recent_slugs: HashSet<String> = recent_slugs_list.into_iter().collect();

    let rules_recent_list = store.rules_used_since(workspace_key, lookback_start)?;
    let rules_used_recent_slugs: HashSet<String> = rules_recent_list.into_iter().collect();

    let skill_files_on_disk = scan_skill_files(workspace_root, window_end_ms)?;
    let rule_files_on_disk = scan_rule_files(workspace_root, window_end_ms)?;
    let file_facts = latest_file_facts(store, workspace_key)?;

    let aggregates = build_aggregates(&events);

    Ok(Inputs {
        window_start_ms,
        window_end_ms,
        events,
        files_touched,
        skills_used,
        tool_spans,
        skills_used_recent_slugs,
        usage_lookback_ms: USAGE_LOOKBACK_MIN_DAYS * 86_400_000,
        skill_files_on_disk,
        rule_files_on_disk,
        rules_used_recent_slugs,
        file_facts,
        aggregates,
    })
}

/// Same as [`load_inputs`], with optional `remote_events` + local merge when `DataSource` is not local.
#[allow(clippy::too_many_arguments)]
pub fn load_inputs_for_data_source(
    store: &Store,
    workspace_root: &Path,
    workspace_key: &str,
    start_ms: u64,
    end_ms: u64,
    source: DataSource,
    team_id: Option<&str>,
    workspace_hash: Option<&str>,
) -> Result<Inputs> {
    match source {
        DataSource::Local => load_inputs(store, workspace_root, workspace_key, start_ms, end_ms),
        DataSource::Provider => {
            if let (Some(t), Some(wh)) = (team_id, workspace_hash) {
                load_inputs_from_remote_cache(
                    store,
                    workspace_root,
                    workspace_key,
                    start_ms,
                    end_ms,
                    t,
                    wh,
                )
            } else {
                load_inputs(store, workspace_root, workspace_key, start_ms, end_ms)
            }
        }
        DataSource::Mixed => {
            let mut i = load_inputs(store, workspace_root, workspace_key, start_ms, end_ms)?;
            if let (Some(t), Some(wh)) = (team_id, workspace_hash) {
                for raw in store.list_remote_event_jsons(t, wh)? {
                    let o: OutboundEvent = serde_json::from_str(&raw)?;
                    if o.ts_ms < start_ms || o.ts_ms > end_ms {
                        continue;
                    }
                    i.events
                        .push(session_event_from_outbound(&o, workspace_key));
                }
                i.events.sort_by(|(a, ea), (b, eb)| {
                    ea.ts_ms
                        .cmp(&eb.ts_ms)
                        .then_with(|| a.id.cmp(&b.id))
                        .then_with(|| ea.seq.cmp(&eb.seq))
                });
                i.aggregates = build_aggregates(&i.events);
            }
            Ok(i)
        }
    }
}

fn event_kind_from_outbound(s: &str) -> EventKind {
    match s {
        "tool_call" => EventKind::ToolCall,
        "tool_result" => EventKind::ToolResult,
        "message" => EventKind::Message,
        "error" => EventKind::Error,
        "cost" => EventKind::Cost,
        "hook" => EventKind::Hook,
        _ => EventKind::Message,
    }
}

fn event_source_from_outbound(s: &str) -> EventSource {
    match s {
        "tail" => EventSource::Tail,
        "proxy" => EventSource::Proxy,
        "hook" => EventSource::Hook,
        _ => EventSource::Hook,
    }
}

fn session_event_from_outbound(o: &OutboundEvent, workspace_key: &str) -> (SessionRecord, Event) {
    let sid = format!("remote:{}", o.session_id_hash);
    let session = SessionRecord {
        id: sid.clone(),
        agent: o.agent.clone(),
        model: Some(o.model.clone()),
        workspace: workspace_key.to_string(),
        started_at_ms: o.ts_ms,
        ended_at_ms: None,
        status: SessionStatus::Done,
        trace_path: String::new(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
    };
    let event = Event {
        session_id: sid,
        seq: o.event_seq,
        ts_ms: o.ts_ms,
        ts_exact: true,
        kind: event_kind_from_outbound(&o.kind),
        source: event_source_from_outbound(&o.source),
        tool: o.tool.clone(),
        tool_call_id: o.tool_call_id.clone(),
        tokens_in: o.tokens_in,
        tokens_out: o.tokens_out,
        reasoning_tokens: o.reasoning_tokens,
        cost_usd_e6: o.cost_usd_e6,
        payload: o.payload.clone(),
    };
    (session, event)
}

fn load_inputs_from_remote_cache(
    store: &Store,
    workspace_root: &Path,
    workspace_key: &str,
    start_ms: u64,
    end_ms: u64,
    team_id: &str,
    workspace_hash: &str,
) -> Result<Inputs> {
    let mut events = Vec::new();
    for raw in store.list_remote_event_jsons(team_id, workspace_hash)? {
        let o: OutboundEvent = serde_json::from_str(&raw)?;
        if o.ts_ms < start_ms || o.ts_ms > end_ms {
            continue;
        }
        events.push(session_event_from_outbound(&o, workspace_key));
    }
    events.sort_by(|(a, ea), (b, eb)| {
        ea.ts_ms
            .cmp(&eb.ts_ms)
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| ea.seq.cmp(&eb.seq))
    });
    let skill_files_on_disk = scan_skill_files(workspace_root, end_ms)?;
    let rule_files_on_disk = scan_rule_files(workspace_root, end_ms)?;
    let lookback_start = end_ms.saturating_sub(USAGE_LOOKBACK_MIN_DAYS * 86_400_000);
    let recent_slugs_list = store.skills_used_since(workspace_key, lookback_start)?;
    let skills_used_recent_slugs: HashSet<String> = recent_slugs_list.into_iter().collect();
    let rules_recent_list = store.rules_used_since(workspace_key, lookback_start)?;
    let rules_used_recent_slugs: HashSet<String> = rules_recent_list.into_iter().collect();
    let file_facts = latest_file_facts(store, workspace_key)?;
    let aggregates = build_aggregates(&events);
    Ok(Inputs {
        window_start_ms: start_ms,
        window_end_ms: end_ms,
        events,
        files_touched: vec![],
        skills_used: vec![],
        tool_spans: vec![],
        skills_used_recent_slugs,
        usage_lookback_ms: USAGE_LOOKBACK_MIN_DAYS * 86_400_000,
        skill_files_on_disk,
        rule_files_on_disk,
        rules_used_recent_slugs,
        file_facts,
        aggregates,
    })
}

fn latest_file_facts(
    store: &Store,
    workspace: &str,
) -> Result<HashMap<String, crate::metrics::types::FileFact>> {
    let Some(snapshot) = store.latest_repo_snapshot(workspace)? else {
        return Ok(HashMap::new());
    };
    let facts = store.file_facts_for_snapshot(&snapshot.id)?;
    Ok(facts
        .into_iter()
        .map(|fact| (fact.path.clone(), fact))
        .collect())
}

/// `.cursor/skills/<slug>/SKILL.md` on disk.
pub fn scan_skill_files(workspace_root: &Path, now_ms: u64) -> Result<Vec<SkillFileOnDisk>> {
    let skills_dir = workspace_root.join(".cursor/skills");
    if !skills_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&skills_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();
        let skill_md = entry.path().join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let meta = fs::metadata(&skill_md)?;
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(now_ms);
        out.push(SkillFileOnDisk {
            slug,
            size_bytes: meta.len(),
            mtime_ms,
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

/// `.cursor/rules/*.mdc` files (stem = rule id).
pub fn scan_rule_files(workspace_root: &Path, now_ms: u64) -> Result<Vec<SkillFileOnDisk>> {
    let rules_dir = workspace_root.join(".cursor/rules");
    if !rules_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&rules_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("mdc"))
        {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let slug = stem.to_string();
        let meta = fs::metadata(&path)?;
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(now_ms);
        out.push(SkillFileOnDisk {
            slug,
            size_bytes: meta.len(),
            mtime_ms,
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

fn build_aggregates(events: &[(SessionRecord, crate::core::event::Event)]) -> RetroAggregates {
    let mut agg = RetroAggregates::default();
    let mut model_once = HashSet::new();
    for (s, e) in events {
        agg.unique_session_ids.insert(s.id.clone());
        if model_once.insert(s.id.clone()) {
            let mkey = s.model.clone().unwrap_or_else(|| "unknown".into());
            *agg.model_session_counts.entry(mkey).or_default() += 1;
        }
        if let Some(ref t) = e.tool {
            *agg.tool_event_counts.entry(t.clone()).or_default() += 1;
            if let Some(c) = e.cost_usd_e6 {
                *agg.tool_cost_usd_e6.entry(t.clone()).or_default() += c;
            }
        }
        if let Some(c) = e.cost_usd_e6 {
            agg.total_cost_usd_e6 += c;
        }
    }
    agg
}

/// Collect bet ids from existing markdown reports (`###` headings embed id in body or title line).
pub fn prior_bet_fingerprints(reports_dir: &Path) -> Result<HashSet<String>> {
    let mut out = HashSet::new();
    if !reports_dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(reports_dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        let raw = fs::read_to_string(&p).unwrap_or_default();
        for line in raw.lines() {
            let l = line.trim();
            let Some(rest) = l.strip_prefix("### ") else {
                continue;
            };
            let Some(open) = rest.rfind('(') else {
                continue;
            };
            let Some(close) = rest.rfind(')') else {
                continue;
            };
            if close <= open + 1 {
                continue;
            }
            let id = rest[open + 1..close].trim();
            if id.starts_with('H') && id.contains(':') {
                out.insert(id.to_string());
            }
        }
    }
    Ok(out)
}
