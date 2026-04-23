// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure smart-metric report builder.

use crate::metrics::types::{MetricsReport, RankedFile, RankedTool, ToolSpanView};
use crate::store::Store;
use anyhow::Result;
use std::collections::HashMap;

pub fn build_report(store: &Store, workspace: &str, days: u32) -> Result<MetricsReport> {
    let snapshot = store.latest_repo_snapshot(workspace)?;
    let facts = snapshot
        .as_ref()
        .map(|snap| store.file_facts_for_snapshot(&snap.id))
        .transpose()?
        .unwrap_or_default();
    let end_ms = now_ms();
    let start_ms = end_ms.saturating_sub(days as u64 * 86_400_000);
    let spans = store.tool_spans_in_window(workspace, start_ms, end_ms)?;
    Ok(MetricsReport {
        snapshot,
        hottest_files: top_files(&facts, |f| f.churn_30d as u64 * f.complexity_total as u64),
        most_changed_files: top_files(&facts, |f| f.churn_30d as u64),
        most_complex_files: top_files(&facts, |f| f.complexity_total as u64),
        highest_risk_files: top_files(&facts, |f| {
            f.churn_30d as u64 * f.authors_90d as u64 * f.complexity_total as u64
        }),
        slowest_tools: rank_tools(&spans, ToolRankMode::Latency),
        highest_token_tools: rank_tools(&spans, ToolRankMode::Tokens),
        highest_reasoning_tools: rank_tools(&spans, ToolRankMode::Reasoning),
        agent_pain_hotspots: pain_hotspots(&facts, &spans),
    })
}

fn top_files<F>(facts: &[crate::metrics::types::FileFact], value: F) -> Vec<RankedFile>
where
    F: Fn(&crate::metrics::types::FileFact) -> u64,
{
    let mut out = facts
        .iter()
        .map(|fact| RankedFile {
            path: fact.path.clone(),
            value: value(fact),
            complexity_total: fact.complexity_total,
            churn_30d: fact.churn_30d,
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.path.cmp(&b.path)));
    out.truncate(10);
    out
}

fn pain_hotspots(
    facts: &[crate::metrics::types::FileFact],
    spans: &[ToolSpanView],
) -> Vec<RankedFile> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for span in spans {
        for path in &span.paths {
            *counts.entry(path.clone()).or_default() += 1;
        }
    }
    let mut out = facts
        .iter()
        .map(|fact| RankedFile {
            path: fact.path.clone(),
            value: counts.get(&fact.path).copied().unwrap_or(0) * fact.complexity_total as u64,
            complexity_total: fact.complexity_total,
            churn_30d: fact.churn_30d,
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.path.cmp(&b.path)));
    out.truncate(10);
    out
}

enum ToolRankMode {
    Latency,
    Tokens,
    Reasoning,
}

fn rank_tools(spans: &[ToolSpanView], mode: ToolRankMode) -> Vec<RankedTool> {
    let mut by_tool: HashMap<String, Vec<&ToolSpanView>> = HashMap::new();
    for span in spans {
        by_tool.entry(span.tool.clone()).or_default().push(span);
    }
    let mut out = by_tool
        .into_iter()
        .map(|(tool, items)| {
            let mut latencies = items
                .iter()
                .filter_map(|span| span.lead_time_ms)
                .collect::<Vec<_>>();
            latencies.sort_unstable();
            let total_tokens = items
                .iter()
                .map(|span| {
                    span.tokens_in.unwrap_or(0) as u64
                        + span.tokens_out.unwrap_or(0) as u64
                        + span.reasoning_tokens.unwrap_or(0) as u64
                })
                .sum::<u64>();
            let total_reasoning_tokens = items
                .iter()
                .map(|span| span.reasoning_tokens.unwrap_or(0) as u64)
                .sum::<u64>();
            RankedTool {
                tool,
                calls: items.len() as u64,
                p50_ms: percentile(&latencies, 50),
                p95_ms: percentile(&latencies, 95),
                total_tokens,
                total_reasoning_tokens,
            }
        })
        .collect::<Vec<_>>();
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

fn percentile(values: &[u64], pct: usize) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let idx = ((values.len() - 1) * pct) / 100;
    Some(values[idx])
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
