use kaizen::metrics::report::build_report;
use kaizen::metrics::types::{FileFact, MetricsReport, RankedFile, RankedTool, ToolSpanView};
use kaizen::store::Store;
use std::collections::HashMap;

mod metrics_report_fixture;
use metrics_report_fixture::{events, facts, now_ms, session, snapshot};

#[test]
fn compact_report_matches_materialized_report() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = Store::open(&dir.path().join("kaizen.db"))?;
    let workspace = "/ws";
    let now = now_ms();
    store.upsert_session(&session("s1", workspace, now))?;
    store.save_repo_snapshot(&snapshot(workspace, now), &facts(), &[])?;
    for event in events("s1", now) {
        store.append_event(&event)?;
    }
    store.flush_projector_session("s1", now)?;

    let compact = build_report(&store, workspace, 7)?;
    let materialized = materialized_report(&store, workspace, 7)?;
    assert_eq!(compact.hottest_files, materialized.hottest_files);
    assert_eq!(compact.most_changed_files, materialized.most_changed_files);
    assert_eq!(compact.most_complex_files, materialized.most_complex_files);
    assert_eq!(compact.highest_risk_files, materialized.highest_risk_files);
    assert_eq!(compact.slowest_tools, materialized.slowest_tools);
    assert_eq!(
        compact.highest_token_tools,
        materialized.highest_token_tools
    );
    assert_eq!(
        compact.highest_reasoning_tools,
        materialized.highest_reasoning_tools
    );
    assert_eq!(
        compact.agent_pain_hotspots,
        materialized.agent_pain_hotspots
    );
    Ok(())
}

fn materialized_report(store: &Store, workspace: &str, days: u32) -> anyhow::Result<MetricsReport> {
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
        slowest_tools: rank_tools(&spans, Rank::Latency),
        highest_token_tools: rank_tools(&spans, Rank::Tokens),
        highest_reasoning_tools: rank_tools(&spans, Rank::Reasoning),
        agent_pain_hotspots: pain_hotspots(&facts, &spans),
    })
}

fn top_files<F>(facts: &[FileFact], value: F) -> Vec<RankedFile>
where
    F: Fn(&FileFact) -> u64,
{
    let mut out = facts
        .iter()
        .map(|f| RankedFile {
            path: f.path.clone(),
            value: value(f),
            complexity_total: f.complexity_total,
            churn_30d: f.churn_30d,
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.path.cmp(&b.path)));
    out.truncate(10);
    out
}

fn pain_hotspots(facts: &[FileFact], spans: &[ToolSpanView]) -> Vec<RankedFile> {
    let counts = spans.iter().fold(HashMap::new(), |mut acc, span| {
        span.paths
            .iter()
            .for_each(|path| *acc.entry(path.clone()).or_insert(0_u64) += 1);
        acc
    });
    top_files(facts, |f| {
        counts.get(&f.path).copied().unwrap_or(0) * f.complexity_total as u64
    })
}

enum Rank {
    Latency,
    Tokens,
    Reasoning,
}

fn rank_tools(spans: &[ToolSpanView], mode: Rank) -> Vec<RankedTool> {
    let mut by_tool: HashMap<String, Vec<&ToolSpanView>> = HashMap::new();
    spans
        .iter()
        .for_each(|span| by_tool.entry(span.tool.clone()).or_default().push(span));
    let mut out = by_tool
        .into_iter()
        .map(|(tool, rows)| ranked_tool(tool, rows))
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        rank_value(b, &mode)
            .cmp(&rank_value(a, &mode))
            .then_with(|| a.tool.cmp(&b.tool))
    });
    out.truncate(10);
    out
}

fn ranked_tool(tool: String, rows: Vec<&ToolSpanView>) -> RankedTool {
    let mut latencies = rows
        .iter()
        .filter_map(|span| span.lead_time_ms)
        .collect::<Vec<_>>();
    latencies.sort_unstable();
    RankedTool {
        tool,
        calls: rows.len() as u64,
        p50_ms: percentile(&latencies, 50),
        p95_ms: percentile(&latencies, 95),
        total_tokens: rows.iter().map(total_tokens).sum(),
        total_reasoning_tokens: rows
            .iter()
            .map(|span| span.reasoning_tokens.unwrap_or(0) as u64)
            .sum(),
    }
}

fn rank_value(row: &RankedTool, mode: &Rank) -> u64 {
    match mode {
        Rank::Latency => row.p95_ms.unwrap_or(0),
        Rank::Tokens => row.total_tokens,
        Rank::Reasoning => row.total_reasoning_tokens,
    }
}

fn total_tokens(span: &&ToolSpanView) -> u64 {
    span.tokens_in.unwrap_or(0) as u64
        + span.tokens_out.unwrap_or(0) as u64
        + span.reasoning_tokens.unwrap_or(0) as u64
}

fn percentile(values: &[u64], pct: usize) -> Option<u64> {
    (!values.is_empty()).then(|| values[((values.len() - 1) * pct) / 100])
}
