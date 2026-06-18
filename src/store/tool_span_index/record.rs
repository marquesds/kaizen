// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{SpanBuilder, ToolSpanRecord, span_end, span_start};

impl ToolSpanRecord {
    pub(crate) fn from_builder(span: &SpanBuilder) -> Self {
        let started = span_start(span);
        let ended = span_end(span);
        Self {
            span_id: span.span_id.clone(),
            session_id: span.session_id.clone(),
            tool: span.tool.clone(),
            tool_call_id: span.tool_call_id.clone(),
            status: status(started, ended).to_owned(),
            started_at_ms: started,
            ended_at_ms: ended,
            lead_time_ms: lead_time(span),
            tokens_in: span.tokens_in,
            tokens_out: span.tokens_out,
            reasoning_tokens: span.reasoning_tokens,
            cost_usd_e6: span.cost_usd_e6,
            paths: span.paths.iter().cloned().collect(),
            parent_span_id: span.parent_span_id.clone(),
            depth: span.depth,
            subtree_cost_usd_e6: span.subtree_cost_usd_e6,
            subtree_token_count: span.subtree_token_count,
        }
    }
}

fn status(started: Option<u64>, ended: Option<u64>) -> &'static str {
    if started.is_some() && ended.is_some() {
        "done"
    } else {
        "orphaned"
    }
}

fn lead_time(span: &SpanBuilder) -> Option<u64> {
    elapsed(span.hook_start_ms, span.hook_end_ms).or_else(|| exact_call_lead_time(span))
}

fn exact_call_lead_time(span: &SpanBuilder) -> Option<u64> {
    if !span.call_start_exact || !span.result_end_exact {
        return None;
    }
    elapsed(span.call_start_ms, span.result_end_ms)
}

fn elapsed(start: Option<u64>, end: Option<u64>) -> Option<u64> {
    start.zip(end).map(|(start, end)| end.saturating_sub(start))
}
