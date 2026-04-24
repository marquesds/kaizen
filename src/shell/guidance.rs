// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen guidance` — skill/rule adoption and cost proxy from observed payload references.

use crate::core::config;
use crate::core::data_source::DataSource;
use crate::retro::inputs::{scan_rule_files, scan_skill_files};
use crate::shell::cli::{maybe_refresh_store, workspace_path};
use crate::shell::remote_pull::maybe_telemetry_pull;
use crate::store::{GuidanceKind, GuidanceReport, Store};
use anyhow::Result;
use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;

/// `(start_ms, end_ms)` for a trailing `days` window from now.
pub fn trailing_window_ms(days: u32) -> (u64, u64) {
    let end_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let start_ms = end_ms.saturating_sub((days as u64).saturating_mul(86_400_000));
    (start_ms, end_ms)
}

/// Build guidance report after optional agent rescan.
pub fn build_guidance_report(
    store: &Store,
    workspace_root: &Path,
    workspace_key: &str,
    days: u32,
) -> Result<GuidanceReport> {
    let (start_ms, end_ms) = trailing_window_ms(days);
    let skill_files = scan_skill_files(workspace_root, end_ms)?;
    let rule_files = scan_rule_files(workspace_root, end_ms)?;
    let skill_slugs: HashSet<String> = skill_files.into_iter().map(|s| s.slug).collect();
    let rule_slugs: HashSet<String> = rule_files.into_iter().map(|s| s.slug).collect();
    store.guidance_report(workspace_key, start_ms, end_ms, &skill_slugs, &rule_slugs)
}

pub fn guidance_text(
    workspace: Option<&Path>,
    days: u32,
    json_out: bool,
    refresh: bool,
    source: DataSource,
) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let ws_str = ws.to_string_lossy().to_string();
    let cfg = config::load(&ws)?;
    maybe_telemetry_pull(&ws, &store, &cfg, source, refresh)?;
    maybe_refresh_store(&ws, &store, refresh)?;
    let mut report = build_guidance_report(&store, &ws, &ws_str, days)?;
    if source != DataSource::Local
        && let Ok(Some(agg)) = crate::shell::remote_observe::try_remote_event_agg(&store, &cfg, &ws)
    {
        report =
            crate::shell::remote_observe::merge_guidance_sessions_in_window(report, &agg, source);
    }
    if json_out {
        return Ok(serde_json::to_string_pretty(&report)?);
    }
    Ok(format_human(&report, days))
}

pub fn cmd_guidance(
    workspace: Option<&Path>,
    days: u32,
    json_out: bool,
    refresh: bool,
    source: DataSource,
) -> Result<()> {
    print!(
        "{}",
        guidance_text(workspace, days, json_out, refresh, source)?
    );
    Ok(())
}

/// Short block for `kaizen insights` (top observed skills/rules by session count).
pub fn format_guidance_teaser(
    store: &Store,
    workspace_root: &Path,
    workspace_key: &str,
    days: u32,
) -> Result<String> {
    let report = build_guidance_report(store, workspace_root, workspace_key, days)?;
    let mut s = String::new();
    let _ = writeln!(
        &mut s,
        "Guidance (observed .cursor/skills + .cursor/rules path refs, last {days}d)"
    );
    let _ = writeln!(
        &mut s,
        "  Sessions in window: {} · workspace avg $/session: {}",
        report.sessions_in_window,
        report
            .workspace_avg_cost_per_session_usd
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "n/a".into())
    );
    let mut active: Vec<_> = report.rows.iter().filter(|r| r.sessions > 0).collect();
    active.sort_by_key(|r| std::cmp::Reverse(r.sessions));
    if active.is_empty() {
        let _ = writeln!(
            &mut s,
            "  (no skill/rule path references in payloads — run agents that read SKILL.md / .mdc)"
        );
    } else {
        let _ = writeln!(&mut s, "  Top by sessions:");
        for r in active.iter().take(3) {
            let kind = match r.kind {
                GuidanceKind::Skill => "skill",
                GuidanceKind::Rule => "rule",
            };
            let _ = writeln!(
                &mut s,
                "    · {} `{}` — {} sessions ({:.1}% of window)",
                kind, r.id, r.sessions, r.sessions_pct
            );
        }
    }
    let _ = writeln!(&mut s, "  Full table: `kaizen guidance --days {days}`");
    Ok(s)
}

fn format_human(report: &GuidanceReport, days: u32) -> String {
    let mut s = String::new();
    let _ = writeln!(
        &mut s,
        "kaizen guidance — {} (last {}d, observed payload refs only)",
        report.workspace, days
    );
    let _ = writeln!(&mut s);
    let _ = writeln!(&mut s, "Sessions in window: {}", report.sessions_in_window);
    let _ = writeln!(
        &mut s,
        "Workspace avg $/session: {}",
        report
            .workspace_avg_cost_per_session_usd
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "n/a".into())
    );
    let _ = writeln!(&mut s);
    let _ = writeln!(
        &mut s,
        "{:<6} {:<24} {:>9} {:>8} {:>10} {:>10}  note",
        "kind", "id", "sessions", "%window", "avg$/sess", "vs avg"
    );
    for r in &report.rows {
        let kind = match r.kind {
            GuidanceKind::Skill => "skill",
            GuidanceKind::Rule => "rule",
        };
        let avg = r
            .avg_cost_per_session_usd
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "n/a".into());
        let vs = r
            .vs_workspace_avg_cost_per_session_usd
            .map(|v| format!("{:+.4}", v))
            .unwrap_or_else(|| "n/a".into());
        let note = if r.sessions == 0 && r.on_disk {
            "unused on disk"
        } else if !r.on_disk && r.sessions > 0 {
            "not in workspace inventory"
        } else {
            ""
        };
        let _ = writeln!(
            &mut s,
            "{:<6} {:<24} {:>9} {:>7.1}% {:>10} {:>10}  {}",
            kind, r.id, r.sessions, r.sessions_pct, avg, vs, note
        );
    }
    let _ = writeln!(&mut s);
    let _ = writeln!(
        &mut s,
        "Counts reflect path strings in ingested tool payloads, not silent Cursor rule injection."
    );
    s
}
