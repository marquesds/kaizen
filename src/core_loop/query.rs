// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::event::{Event, SessionRecord};
use crate::core_loop::TraceHit;
use crate::core_loop::query_meta::Meta;
use crate::core_loop::query_syntax::{Field, Op, QueryExpr, Term};
use crate::search::{kind_label, tokens_total};
use crate::store::Store;
use anyhow::Result;

pub fn is_structured(raw: &str) -> bool {
    crate::core_loop::query_syntax::is_structured(raw)
}

pub fn parse(raw: &str) -> Result<QueryExpr> {
    crate::core_loop::query_syntax::parse(raw)
}

pub fn run(
    store: &Store,
    workspace: &str,
    raw: &str,
    start_ms: u64,
    limit: usize,
) -> Result<Vec<TraceHit>> {
    let expr = parse(raw)?;
    let meta = Meta::load(store, start_ms)?;
    let rows = store.workspace_events(workspace)?;
    Ok(rows
        .into_iter()
        .filter_map(|r| hit_if(&expr, &meta, r))
        .take(limit)
        .collect())
}

fn hit_if(expr: &QueryExpr, meta: &Meta, row: (SessionRecord, Event)) -> Option<TraceHit> {
    let (session, event) = row;
    expr.terms
        .iter()
        .all(|t| matches_term(t, meta, &session, &event))
        .then(|| hit(session, event))
}

fn hit(session: SessionRecord, event: Event) -> TraceHit {
    TraceHit {
        session_id: session.id,
        seq: Some(event.seq),
        ts_ms: event.ts_ms,
        agent: session.agent,
        kind: kind_label(&event.kind).unwrap_or("unknown").into(),
        summary: tool(&event)
            .or_else(|| text(&event.payload))
            .unwrap_or_default(),
    }
}

fn matches_term(t: &Term, meta: &Meta, s: &SessionRecord, e: &Event) -> bool {
    match t.field {
        Field::Agent => eq(&s.agent, &t.value),
        Field::Model => s.model.as_deref().is_some_and(|v| eq(v, &t.value)),
        Field::Kind => kind_label(&e.kind).is_some_and(|v| eq(v, &t.value)),
        Field::Tool => tool(e).as_deref().is_some_and(|v| eq(v, &t.value)),
        Field::Path => paths(e).iter().any(|p| p.contains(&t.value)),
        Field::Skill => skills(e).iter().any(|p| p.contains(&t.value)),
        Field::TokensTotal => cmp(tokens_total(e) as f64, t),
        Field::CostUsd => cmp(e.cost_usd_e6.unwrap_or(0) as f64 / 1_000_000.0, t),
        Field::EvalScore => meta.eval(&s.id).is_some_and(|v| cmp(v, t)),
        Field::FeedbackLabel => meta.feedback(&s.id).is_some_and(|v| eq(v, &t.value)),
        Field::Prompt => s
            .prompt_fingerprint
            .as_deref()
            .is_some_and(|v| v.starts_with(&t.value)),
        Field::Status => eq(status(s), &t.value),
        Field::SpanKind => meta.span_kind(&s.id, &t.value),
    }
}

fn cmp(n: f64, t: &Term) -> bool {
    let Ok(v) = t.value.parse::<f64>() else {
        return false;
    };
    match t.op {
        Op::Eq => n == v,
        Op::Gt => n > v,
        Op::Gte => n >= v,
        Op::Lt => n < v,
        Op::Lte => n <= v,
    }
}

fn eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn status(s: &SessionRecord) -> &'static str {
    match s.status {
        crate::core::event::SessionStatus::Running => "running",
        crate::core::event::SessionStatus::Waiting => "waiting",
        crate::core::event::SessionStatus::Idle => "idle",
        crate::core::event::SessionStatus::Done => "done",
    }
}

fn text(v: &serde_json::Value) -> Option<String> {
    v.get("text").and_then(|v| v.as_str()).map(str::to_string)
}

fn tool(e: &Event) -> Option<String> {
    e.tool
        .clone()
        .or_else(|| crate::store::tool_span_index::hook_tool(&e.payload))
}

fn paths(e: &Event) -> Vec<String> {
    crate::store::event_index::paths_from_event_payload(&e.payload)
}

fn skills(e: &Event) -> Vec<String> {
    crate::store::event_index::skills_from_event_json(&e.payload)
}
