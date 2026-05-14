// SPDX-License-Identifier: AGPL-3.0-or-later
//! Smart-metric report builder.

use crate::metrics::types::{MetricsReport, RankedTool};
use crate::store::Store;
use anyhow::Result;

pub fn build_report(store: &Store, workspace: &str, days: u32) -> Result<MetricsReport> {
    let snapshot = store.latest_repo_snapshot(workspace)?;
    let end_ms = now_ms();
    let start_ms = end_ms.saturating_sub(days as u64 * 86_400_000);
    let tools = store.tool_rank_rows_in_window(workspace, start_ms, end_ms)?;
    let files = snapshot.as_ref().map(|snap| snap.id.as_str());
    let hottest = files
        .map(|id| store.hottest_files_for_snapshot(id))
        .transpose()?;
    let changed = files
        .map(|id| store.most_changed_files_for_snapshot(id))
        .transpose()?;
    let complex = files
        .map(|id| store.most_complex_files_for_snapshot(id))
        .transpose()?;
    let risky = files
        .map(|id| store.highest_risk_files_for_snapshot(id))
        .transpose()?;
    let pain = files
        .map(|id| store.pain_hotspots_for_snapshot(id, workspace, start_ms, end_ms))
        .transpose()?;
    Ok(MetricsReport {
        snapshot,
        hottest_files: hottest.unwrap_or_default(),
        most_changed_files: changed.unwrap_or_default(),
        most_complex_files: complex.unwrap_or_default(),
        highest_risk_files: risky.unwrap_or_default(),
        slowest_tools: top_tools(tools.clone(), ToolRankMode::Latency),
        highest_token_tools: top_tools(tools.clone(), ToolRankMode::Tokens),
        highest_reasoning_tools: top_tools(tools, ToolRankMode::Reasoning),
        agent_pain_hotspots: pain.unwrap_or_default(),
    })
}

enum ToolRankMode {
    Latency,
    Tokens,
    Reasoning,
}

fn top_tools(mut out: Vec<RankedTool>, mode: ToolRankMode) -> Vec<RankedTool> {
    out.sort_by(|a, b| {
        tool_rank_value(b, &mode)
            .cmp(&tool_rank_value(a, &mode))
            .then_with(|| a.tool.cmp(&b.tool))
    });
    out.truncate(10);
    out
}

fn tool_rank_value(tool: &RankedTool, mode: &ToolRankMode) -> u64 {
    match mode {
        ToolRankMode::Latency => tool.p95_ms.unwrap_or(0),
        ToolRankMode::Tokens => tool.total_tokens,
        ToolRankMode::Reasoning => tool.total_reasoning_tokens,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
