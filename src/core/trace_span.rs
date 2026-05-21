// SPDX-License-Identifier: AGPL-3.0-or-later
//! Additive trace-span model for Datadog/OTLP-style session timelines.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceSpanKind {
    Session,
    Agent,
    Step,
    Llm,
    Tool,
    Permission,
}

impl TraceSpanKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Agent => "agent",
            Self::Step => "step",
            Self::Llm => "llm",
            Self::Tool => "tool",
            Self::Permission => "permission",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "session" => Self::Session,
            "agent" => Self::Agent,
            "step" => Self::Step,
            "tool" => Self::Tool,
            "permission" => Self::Permission,
            _ => Self::Llm,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpanRecord {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub session_id: String,
    pub kind: TraceSpanKind,
    pub name: String,
    pub status: String,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub duration_ms: Option<u32>,
    pub model: Option<String>,
    pub tool: Option<String>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub reasoning_tokens: Option<u32>,
    pub cost_usd_e6: Option<i64>,
    pub context_used_tokens: Option<u32>,
    pub context_max_tokens: Option<u32>,
    pub payload: Value,
}

impl TraceSpanRecord {
    pub fn llm_proxy(
        session_id: &str,
        seq: u64,
        started_at_ms: u64,
        ended_at_ms: u64,
        model: Option<String>,
        payload: Value,
    ) -> Self {
        let duration = ended_at_ms.saturating_sub(started_at_ms);
        Self {
            span_id: format!("llm-{session_id}-{seq}"),
            trace_id: trace_id_for_session(session_id),
            parent_span_id: Some(format!("step-{session_id}-{seq}")),
            session_id: session_id.to_string(),
            kind: Self::llm_kind(),
            name: "llm.proxy".into(),
            status: "ok".into(),
            started_at_ms: Some(started_at_ms),
            ended_at_ms: Some(ended_at_ms),
            duration_ms: u32::try_from(duration).ok(),
            model,
            tool: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            context_used_tokens: None,
            context_max_tokens: None,
            payload,
        }
    }

    fn llm_kind() -> TraceSpanKind {
        TraceSpanKind::Llm
    }
}

pub fn trace_id_for_session(session_id: &str) -> String {
    let hash = blake3::hash(session_id.as_bytes());
    hex::encode(&hash.as_bytes()[..16])
}

pub fn span_payload(provider: &str, stream: bool, request_id: Option<&str>) -> Value {
    json!({"provider": provider, "stream": stream, "request_id": request_id})
}
