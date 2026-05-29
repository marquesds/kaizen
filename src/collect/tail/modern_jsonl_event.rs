// SPDX-License-Identifier: AGPL-3.0-or-later

use super::modern_jsonl_fields::{Usage, call_id, json, text, timestamp, tool, usage};
use crate::collect::model_from_json;
use crate::core::cost::estimate_tail_event_cost_usd_e6;
use crate::core::event::{Event, EventKind, EventSource};
use serde_json::Value;

struct Ctx<'a> {
    sid: &'a str,
    seq: u64,
    base: u64,
    ts: Option<u64>,
    usage: Usage,
    model: Option<&'a str>,
}

pub(crate) fn parse_common_line(
    _agent: &str,
    sid: &str,
    seq: u64,
    base: u64,
    line: &str,
) -> Option<Event> {
    let v = json(line)?;
    let model = model_from_json::from_value(&v);
    let ctx = Ctx {
        sid,
        seq,
        base,
        ts: timestamp(&v),
        usage: usage(&v),
        model: model.as_deref(),
    };
    content_event(&ctx, &v)
        .or_else(|| tool_calls_event(&ctx, &v))
        .or_else(|| tool_result_event(&ctx, &v))
        .or_else(|| parts_event(&ctx, &v))
        .or_else(|| generic_event(&ctx, &v))
}

fn content_event(ctx: &Ctx<'_>, v: &Value) -> Option<Event> {
    v.pointer("/message/content")?
        .as_array()?
        .iter()
        .find_map(|b| {
            let kind = match text(b, "type")?.as_str() {
                "tool_use" => EventKind::ToolCall,
                "tool_result" => EventKind::ToolResult,
                _ => return None,
            };
            Some(event(ctx, kind, tool(b), call_id(b), b))
        })
}

fn tool_calls_event(ctx: &Ctx<'_>, v: &Value) -> Option<Event> {
    let call = v.get("tool_calls")?.as_array()?.first()?;
    let name = call
        .pointer("/function/name")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(event(ctx, EventKind::ToolCall, name, call_id(call), call))
}

fn tool_result_event(ctx: &Ctx<'_>, v: &Value) -> Option<Event> {
    let role = text(v, "role").or_else(|| text(v, "type"))?;
    matches!(
        role.as_str(),
        "tool" | "tool_result" | "function_call_output"
    )
    .then(|| event(ctx, EventKind::ToolResult, None, call_id(v), v))
}

fn parts_event(ctx: &Ctx<'_>, v: &Value) -> Option<Event> {
    [
        "/parts",
        "/payload/parts",
        "/message/parts",
        "/content/parts",
    ]
    .iter()
    .find_map(|p| part_array(ctx, v.pointer(p)))
    .or_else(|| {
        v.get("candidates")?
            .as_array()?
            .iter()
            .find_map(|c| part_array(ctx, c.pointer("/content/parts")))
    })
}

fn part_array(ctx: &Ctx<'_>, parts: Option<&Value>) -> Option<Event> {
    parts?.as_array()?.iter().find_map(|p| {
        p.get("functionCall")
            .map(|v| (EventKind::ToolCall, v))
            .or_else(|| {
                p.get("functionResponse")
                    .map(|v| (EventKind::ToolResult, v))
            })
            .map(|(kind, body)| event(ctx, kind, tool(body), call_id(body), body))
    })
}

fn generic_event(ctx: &Ctx<'_>, v: &Value) -> Option<Event> {
    let kind = match text(v, "type").or_else(|| text(v, "event"))?.as_str() {
        "tool_call" | "tool-use" | "toolUse" => EventKind::ToolCall,
        "tool_result" | "tool-result" | "toolResult" => EventKind::ToolResult,
        _ => return None,
    };
    Some(event(ctx, kind, tool(v), call_id(v), v))
}

fn event(
    ctx: &Ctx<'_>,
    kind: EventKind,
    tool: Option<String>,
    id: Option<String>,
    payload: &Value,
) -> Event {
    let (tokens_in, tokens_out, reasoning_tokens) = ctx.usage;
    Event {
        session_id: ctx.sid.to_string(),
        seq: ctx.seq,
        ts_ms: ctx.ts.unwrap_or(ctx.base + ctx.seq * 100),
        ts_exact: ctx.ts.is_some(),
        kind,
        source: EventSource::Tail,
        tool,
        tool_call_id: id,
        tokens_in,
        tokens_out,
        reasoning_tokens,
        cost_usd_e6: estimate_tail_event_cost_usd_e6(
            ctx.model,
            tokens_in,
            tokens_out,
            reasoning_tokens,
        ),
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
