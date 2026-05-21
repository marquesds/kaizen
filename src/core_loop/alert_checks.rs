// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{AlertEvent, AlertSeverity};
use crate::store::Store;
use anyhow::Result;

type EventRow = (crate::core::event::SessionRecord, crate::core::event::Event);

pub fn check_builtin(
    store: &Store,
    workspace: &str,
    start_ms: u64,
    now_ms: u64,
) -> Result<Vec<AlertEvent>> {
    let rows = store.workspace_events(workspace)?;
    let mut out = crate::core_loop::alert_cost::cost_spike(store, workspace, start_ms, now_ms)?;
    out.extend(error_rate(store, &rows, start_ms, now_ms)?);
    out.extend(context_pressure(store, &rows, start_ms, now_ms)?);
    out.extend(retry_cascade(store, &rows, start_ms, now_ms)?);
    out.extend(truncation(store, &rows, start_ms, now_ms)?);
    out.extend(feedback_drop(store, start_ms, now_ms)?);
    out.extend(eval_regression(store, start_ms, now_ms)?);
    Ok(out)
}

fn error_rate(
    store: &Store,
    rows: &[EventRow],
    start_ms: u64,
    now_ms: u64,
) -> Result<Vec<AlertEvent>> {
    let n = rows
        .iter()
        .filter(|(_, e)| {
            e.ts_ms >= start_ms && matches!(e.kind, crate::core::event::EventKind::Error)
        })
        .count();
    alert_if(
        store,
        n >= 3,
        "error_rate",
        "three or more error events in window",
        start_ms,
        now_ms,
    )
}

fn context_pressure(
    store: &Store,
    rows: &[EventRow],
    start_ms: u64,
    now_ms: u64,
) -> Result<Vec<AlertEvent>> {
    let n = rows
        .iter()
        .filter(|(_, e)| e.ts_ms >= start_ms && pressure(e))
        .count();
    alert_if(
        store,
        n > 0,
        "context_pressure",
        "one or more events used at least 80% context",
        start_ms,
        now_ms,
    )
}

fn retry_cascade(
    store: &Store,
    rows: &[EventRow],
    start_ms: u64,
    now_ms: u64,
) -> Result<Vec<AlertEvent>> {
    let n: u16 = rows
        .iter()
        .filter(|(_, e)| e.ts_ms >= start_ms)
        .filter_map(|(_, e)| e.retry_count)
        .sum();
    alert_if(
        store,
        n >= 3,
        "rate_limit_cascade",
        "retry count crossed cascade threshold",
        start_ms,
        now_ms,
    )
}

fn truncation(
    store: &Store,
    rows: &[EventRow],
    start_ms: u64,
    now_ms: u64,
) -> Result<Vec<AlertEvent>> {
    let scoped = rows
        .iter()
        .filter(|(_, e)| e.ts_ms >= start_ms)
        .collect::<Vec<_>>();
    let n = scoped
        .iter()
        .filter(|(_, e)| e.stop_reason.as_deref() == Some("max_tokens"))
        .count();
    alert_if(
        store,
        !scoped.is_empty() && n * 10 >= scoped.len(),
        "truncation_rate",
        "max-token stops crossed 10%",
        start_ms,
        now_ms,
    )
}

fn feedback_drop(store: &Store, start_ms: u64, now_ms: u64) -> Result<Vec<AlertEvent>> {
    let n = store
        .list_feedback_in_window(start_ms, now_ms)?
        .into_iter()
        .filter(|r| {
            r.label.as_ref().is_some_and(|l| {
                matches!(
                    l,
                    crate::feedback::types::FeedbackLabel::Bad
                        | crate::feedback::types::FeedbackLabel::Regression
                )
            })
        })
        .count();
    alert_if(
        store,
        n >= 2,
        "feedback_score_drop",
        "bad/regression feedback crossed threshold",
        start_ms,
        now_ms,
    )
}

fn eval_regression(store: &Store, start_ms: u64, now_ms: u64) -> Result<Vec<AlertEvent>> {
    let rows = store.list_evals_in_window(start_ms, now_ms)?;
    let mean = rows.iter().map(|r| r.score).sum::<f64>() / rows.len().max(1) as f64;
    alert_if(
        store,
        rows.len() >= 3 && mean < 0.4,
        "eval_regression",
        "mean eval score below 0.40",
        start_ms,
        now_ms,
    )
}

fn alert_if(
    store: &Store,
    ok: bool,
    name: &str,
    msg: &str,
    start_ms: u64,
    now_ms: u64,
) -> Result<Vec<AlertEvent>> {
    if !ok {
        return Ok(vec![]);
    }
    crate::core_loop::alerts::emit(
        store,
        &format!("builtin:{name}:{start_ms}"),
        name,
        AlertSeverity::Warning,
        msg,
        None,
        now_ms,
    )
    .map(|a| vec![a])
}

fn pressure(e: &crate::core::event::Event) -> bool {
    matches!((e.context_used_tokens, e.context_max_tokens), (Some(u), Some(m)) if m > 0 && u * 100 >= m * 80)
}
