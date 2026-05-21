// SPDX-License-Identifier: AGPL-3.0-or-later
//! Event mapping for Codex Desktop JSONL lines.

use crate::core::cost::estimate_tail_event_cost_usd_e6;
use crate::core::event::{Event, EventKind, EventSource};
use serde_json::Value;

pub fn parse_modern_line(
    session_id: &str,
    seq: u64,
    base: u64,
    model: Option<&str>,
    line: &str,
) -> Option<Event> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let obj = v.as_object()?;
    let payload = obj.get("payload")?;
    let kind = obj.get("type").and_then(Value::as_str).unwrap_or("");
    let ptype = payload.get("type").and_then(Value::as_str).unwrap_or("");
    match (kind, ptype) {
        ("response_item", "function_call") => tool_call(session_id, seq, base, obj, payload),
        ("response_item", "function_call_output") => {
            tool_result(session_id, seq, base, obj, payload)
        }
        ("event_msg", "token_count") => token_count(session_id, seq, base, model, obj, payload),
        _ => None,
    }
}

fn tool_call(
    session_id: &str,
    seq: u64,
    base: u64,
    obj: &serde_json::Map<String, Value>,
    payload: &Value,
) -> Option<Event> {
    Some(
        event_base(session_id, seq, base, obj, payload, EventKind::ToolCall)
            .with_tool(text(payload, "name"), text(payload, "call_id")),
    )
}

fn tool_result(
    session_id: &str,
    seq: u64,
    base: u64,
    obj: &serde_json::Map<String, Value>,
    payload: &Value,
) -> Option<Event> {
    Some(
        event_base(session_id, seq, base, obj, payload, EventKind::ToolResult)
            .with_tool(None, text(payload, "call_id")),
    )
}

fn token_count(
    session_id: &str,
    seq: u64,
    base: u64,
    model: Option<&str>,
    obj: &serde_json::Map<String, Value>,
    payload: &Value,
) -> Option<Event> {
    let usage = payload.pointer("/info/last_token_usage")?;
    let mut event = event_base(session_id, seq, base, obj, payload, EventKind::Cost);
    event.tokens_in = u32_field(usage, "input_tokens");
    event.tokens_out = u32_field(usage, "output_tokens");
    event.reasoning_tokens = u32_field(usage, "reasoning_output_tokens");
    event.cache_read_tokens = u32_field(usage, "cached_input_tokens");
    event.context_used_tokens = u32_field(usage, "total_tokens").or_else(|| sum_tokens(&event));
    event.context_max_tokens = payload
        .pointer("/info/model_context_window")
        .and_then(as_u32);
    event.cost_usd_e6 = estimate_tail_event_cost_usd_e6(
        model,
        event.tokens_in,
        event.tokens_out,
        event.reasoning_tokens,
    );
    Some(event)
}

fn event_base(
    session_id: &str,
    seq: u64,
    base: u64,
    obj: &serde_json::Map<String, Value>,
    payload: &Value,
    kind: EventKind,
) -> Event {
    let ts = line_ts(obj, payload);
    Event {
        session_id: session_id.to_string(),
        seq,
        ts_ms: ts.unwrap_or(base + seq * 100),
        ts_exact: ts.is_some(),
        kind,
        source: EventSource::Tail,
        tool: None,
        tool_call_id: None,
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: payload.clone(),
    }
}

trait WithTool {
    fn with_tool(self, tool: Option<String>, tool_call_id: Option<String>) -> Self;
}

impl WithTool for Event {
    fn with_tool(mut self, tool: Option<String>, tool_call_id: Option<String>) -> Self {
        self.tool = tool;
        self.tool_call_id = tool_call_id;
        self
    }
}

fn line_ts(obj: &serde_json::Map<String, Value>, payload: &Value) -> Option<u64> {
    ["timestamp_ms", "ts_ms", "created_at_ms", "timestamp"]
        .iter()
        .find_map(|k| obj.get(*k).and_then(crate::collect::tail::value_ts_ms))
        .or_else(|| {
            payload
                .get("timestamp")
                .and_then(crate::collect::tail::value_ts_ms)
        })
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn u32_field(v: &Value, key: &str) -> Option<u32> {
    v.get(key).and_then(as_u32)
}

fn as_u32(v: &Value) -> Option<u32> {
    v.as_u64().and_then(|n| u32::try_from(n).ok())
}

fn sum_tokens(event: &Event) -> Option<u32> {
    let total = event
        .tokens_in
        .unwrap_or(0)
        .saturating_add(event.tokens_out.unwrap_or(0))
        .saturating_add(event.reasoning_tokens.unwrap_or(0));
    (total > 0).then_some(total)
}
