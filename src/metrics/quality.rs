// SPDX-License-Identifier: AGPL-3.0-or-later
//! Capture-quality metrics: field fill rates and trace correlation health.

use crate::store::Store;
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct CaptureQualityReport {
    pub events_total: u64,
    pub proxy_events: u64,
    pub trace_spans_total: u64,
    pub orphan_span_count: u64,
    pub token_coverage_pct: u8,
    pub latency_coverage_pct: u8,
    pub context_coverage_pct: u8,
    pub proxy_correlation_pct: u8,
}

pub fn build_quality_report(
    store: &Store,
    workspace: &str,
    start_ms: u64,
    end_ms: u64,
) -> Result<CaptureQualityReport> {
    let rows = store.capture_quality_rows(workspace, start_ms, end_ms)?;
    let spans = store.trace_span_quality_rows(workspace, start_ms, end_ms)?;
    let proxy_events = rows.iter().filter(|r| r.source == "Proxy").count() as u64;
    let correlated = proxy_events.min(spans.iter().filter(|r| r.kind == "llm").count() as u64);
    Ok(CaptureQualityReport {
        events_total: rows.len() as u64,
        proxy_events,
        trace_spans_total: spans.len() as u64,
        orphan_span_count: spans.iter().filter(|r| r.is_orphan).count() as u64,
        token_coverage_pct: pct(rows.len(), rows.iter().filter(|r| r.has_tokens).count()),
        latency_coverage_pct: pct(rows.len(), rows.iter().filter(|r| r.has_latency).count()),
        context_coverage_pct: pct(rows.len(), rows.iter().filter(|r| r.has_context).count()),
        proxy_correlation_pct: pct(proxy_events as usize, correlated as usize),
    })
}

fn pct(total: usize, good: usize) -> u8 {
    if total == 0 {
        return 0;
    }
    (((good * 100) + (total / 2)) / total).min(100) as u8
}
