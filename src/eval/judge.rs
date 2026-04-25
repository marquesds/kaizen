// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::event::{Event, EventKind, SessionRecord};
use crate::eval::rubric::Rubric;
use crate::eval::types::{EvalRow, JudgeResponse};
use anyhow::{Context, Result, bail};
use serde_json::json;

fn summarise(session: &SessionRecord, events: &[Event]) -> String {
    let tool_seq: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EventKind::ToolCall)
        .filter_map(|e| e.tool.as_deref())
        .collect();
    let errors = events.iter().filter(|e| e.kind == EventKind::Error).count();
    let cost_usd: f64 = events
        .iter()
        .filter_map(|e| e.cost_usd_e6)
        .map(|c| c as f64 / 1_000_000.0)
        .sum();
    let duration_s = session
        .ended_at_ms
        .map(|end| end.saturating_sub(session.started_at_ms) / 1000)
        .unwrap_or(0);
    format!(
        "tools: {}\nerrors: {}\ncost_usd: {:.4}\nduration_s: {}",
        tool_seq.join(", "),
        errors,
        cost_usd,
        duration_s,
    )
}

/// Render the full judge prompt for a session without calling the LLM.
pub fn build_prompt(rubric: &Rubric, session: &SessionRecord, events: &[Event]) -> String {
    let summary = summarise(session, events);
    rubric.prompt_template.replace("{summary}", &summary)
}

pub fn judge_session(
    client: &reqwest::blocking::Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    rubric: &Rubric,
    session: &SessionRecord,
    events: &[Event],
) -> Result<EvalRow> {
    let prompt = build_prompt(rubric, session, events);
    let body = json!({
        "model": model,
        "max_tokens": 256,
        "messages": [{"role": "user", "content": prompt}],
    });
    let url = format!("{}/v1/messages", endpoint.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .context("judge HTTP request failed")?;
    if !resp.status().is_success() {
        bail!("judge returned {}", resp.status());
    }
    parse_judge_response(resp, session, model, rubric)
}

fn parse_judge_response(
    resp: reqwest::blocking::Response,
    session: &SessionRecord,
    model: &str,
    rubric: &Rubric,
) -> Result<EvalRow> {
    let raw: serde_json::Value = resp.json().context("judge response parse")?;
    let text = raw["content"][0]["text"]
        .as_str()
        .context("missing text in judge response")?;
    let jr: JudgeResponse = serde_json::from_str(text).context("judge JSON parse")?;
    let score = jr.score.clamp(0.0, 1.0);
    Ok(EvalRow {
        id: format!("{}:{}", session.id, rubric.id),
        session_id: session.id.clone(),
        judge_model: model.to_owned(),
        rubric_id: rubric.id.to_owned(),
        score,
        rationale: jr.rationale,
        flagged: score < 0.4,
        created_at_ms: now_ms(),
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
