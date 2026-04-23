// SPDX-License-Identifier: AGPL-3.0-or-later
//! Build [`Inputs`] from SQLite + workspace filesystem.

use crate::core::event::SessionRecord;
use crate::retro::types::{Inputs, RetroAggregates, SkillFileOnDisk};
use crate::store::Store;
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

    let skill_files_on_disk = scan_skill_files(workspace_root, window_end_ms)?;
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

fn scan_skill_files(workspace_root: &Path, now_ms: u64) -> Result<Vec<SkillFileOnDisk>> {
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
