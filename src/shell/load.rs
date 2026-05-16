// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen load` — explicit transcript backfill into local workspace stores.

use crate::core::config;
use crate::shell::cli::{AgentScanStats, scan_all_agents_with_stats};
use crate::store::Store;
use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct LoadJson {
    workspace_count: usize,
    totals: AgentScanStats,
    workspaces: Vec<LoadWorkspaceJson>,
}

#[derive(Serialize)]
struct LoadWorkspaceJson {
    workspace: String,
    #[serde(flatten)]
    stats: AgentScanStats,
}

pub fn cmd_load(workspace: Option<&Path>, json: bool) -> Result<()> {
    print!("{}", load_text(workspace, json)?);
    Ok(())
}

pub fn load_text(workspace: Option<&Path>, json: bool) -> Result<String> {
    let roots = load_roots(workspace)?;
    let mut totals = AgentScanStats::default();
    let mut rows = Vec::new();
    for root in roots {
        let stats = load_one(&root)?;
        totals.merge(&stats);
        rows.push(LoadWorkspaceJson {
            workspace: root.to_string_lossy().to_string(),
            stats,
        });
    }
    render_load(rows, totals, json)
}

fn load_roots(workspace: Option<&Path>) -> Result<Vec<PathBuf>> {
    if let Some(path) = workspace {
        return Ok(vec![crate::core::paths::canonical(path)]);
    }
    let roots = crate::core::workspace::machine_workspaces(None)?;
    if roots.is_empty() {
        return crate::core::workspace::resolve(None).map(|p| vec![p]);
    }
    Ok(roots)
}

fn load_one(workspace: &Path) -> Result<AgentScanStats> {
    let cfg = config::load(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(workspace)?)?;
    let ws = workspace.to_string_lossy().to_string();
    scan_all_agents_with_stats(workspace, &cfg, &ws, &store)
}

fn render_load(rows: Vec<LoadWorkspaceJson>, totals: AgentScanStats, json: bool) -> Result<String> {
    if json {
        return Ok(format!(
            "{}\n",
            serde_json::to_string_pretty(&LoadJson {
                workspace_count: rows.len(),
                totals,
                workspaces: rows,
            })?
        ));
    }
    Ok(render_text(&rows, &totals))
}

fn render_text(rows: &[LoadWorkspaceJson], totals: &AgentScanStats) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(&mut out, "Loaded {} workspace(s)", rows.len()).unwrap();
    for row in rows {
        writeln!(
            &mut out,
            "{}: sessions {} events {} agents {}",
            row.workspace,
            row.stats.sessions_upserted,
            row.stats.events_upserted,
            agents_label(&row.stats),
        )
        .unwrap();
    }
    writeln!(
        &mut out,
        "Total: sessions {} events {} agents {}",
        totals.sessions_upserted,
        totals.events_upserted,
        agents_label(totals),
    )
    .unwrap();
    out
}

fn agents_label(stats: &AgentScanStats) -> String {
    if stats.agents.is_empty() {
        "-".into()
    } else {
        stats.agents.iter().cloned().collect::<Vec<_>>().join(",")
    }
}
