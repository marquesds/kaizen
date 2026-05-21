use super::*;

pub(super) fn count_q(conn: &Connection, sql: &str, workspace: &str) -> Result<u64> {
    Ok(conn.query_row(sql, params![workspace], |r| r.get::<_, i64>(0))? as u64)
}

pub(super) fn session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRecord> {
    let status_str: String = row.get(6)?;
    Ok(SessionRecord {
        id: row.get(0)?,
        agent: row.get(1)?,
        model: row.get(2)?,
        workspace: row.get(3)?,
        started_at_ms: row.get::<_, i64>(4)? as u64,
        ended_at_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
        status: status_from_str(&status_str),
        trace_path: row.get(7)?,
        start_commit: row.get(8)?,
        end_commit: row.get(9)?,
        branch: row.get(10)?,
        dirty_start: row.get::<_, Option<i64>>(11)?.map(i64_to_bool),
        dirty_end: row.get::<_, Option<i64>>(12)?.map(i64_to_bool),
        repo_binding_source: empty_to_none(row.get::<_, String>(13)?),
        prompt_fingerprint: row.get(14)?,
        parent_session_id: row.get(15)?,
        agent_version: row.get(16)?,
        os: row.get(17)?,
        arch: row.get(18)?,
        repo_file_count: row.get::<_, Option<i64>>(19)?.map(|v| v as u32),
        repo_total_loc: row.get::<_, Option<i64>>(20)?.map(|v| v as u64),
    })
}

pub(super) fn ranked_file_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RankedFile> {
    Ok(RankedFile {
        path: row.get(0)?,
        value: row.get::<_, i64>(1)? as u64,
        complexity_total: row.get::<_, i64>(2)? as u32,
        churn_30d: row.get::<_, i64>(3)? as u32,
    })
}

pub(super) fn ranked_tool_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RankedTool> {
    Ok(RankedTool {
        tool: row.get(0)?,
        calls: row.get::<_, i64>(1)? as u64,
        p50_ms: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
        p95_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
        total_tokens: row.get::<_, i64>(4)? as u64,
        total_reasoning_tokens: row.get::<_, i64>(5)? as u64,
    })
}

pub(super) fn event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let payload_str: String = row.get(12)?;
    Ok(Event {
        session_id: row.get(0)?,
        seq: row.get::<_, i64>(1)? as u64,
        ts_ms: row.get::<_, i64>(2)? as u64,
        ts_exact: row.get::<_, i64>(3)? != 0,
        kind: kind_from_str(&row.get::<_, String>(4)?),
        source: source_from_str(&row.get::<_, String>(5)?),
        tool: row.get(6)?,
        tool_call_id: row.get(7)?,
        tokens_in: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
        tokens_out: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
        reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
        cost_usd_e6: row.get(11)?,
        payload: serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null),
        stop_reason: row.get(13)?,
        latency_ms: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
        ttft_ms: row.get::<_, Option<i64>>(15)?.map(|v| v as u32),
        retry_count: row.get::<_, Option<i64>>(16)?.map(|v| v as u16),
        context_used_tokens: row.get::<_, Option<i64>>(17)?.map(|v| v as u32),
        context_max_tokens: row.get::<_, Option<i64>>(18)?.map(|v| v as u32),
        cache_creation_tokens: row.get::<_, Option<i64>>(19)?.map(|v| v as u32),
        cache_read_tokens: row.get::<_, Option<i64>>(20)?.map(|v| v as u32),
        system_prompt_tokens: row.get::<_, Option<i64>>(21)?.map(|v| v as u32),
    })
}

pub(super) fn search_tool_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, Event)> {
    Ok((row.get(22)?, event_row(row)?))
}

pub(super) fn session_filter_sql(workspace: &str, filter: &SessionFilter) -> (String, Vec<Value>) {
    let mut clauses = vec!["workspace = ?".to_string()];
    let mut args = vec![Value::Text(workspace.to_string())];
    if let Some(prefix) = filter.agent_prefix.as_deref().filter(|s| !s.is_empty()) {
        clauses.push("lower(agent) LIKE ? ESCAPE '\\'".to_string());
        args.push(Value::Text(format!("{}%", escape_like(prefix))));
    }
    if let Some(status) = &filter.status {
        clauses.push("status = ?".to_string());
        args.push(Value::Text(format!("{status:?}")));
    }
    if let Some(since_ms) = filter.since_ms {
        clauses.push("started_at_ms >= ?".to_string());
        args.push(Value::Integer(since_ms as i64));
    }
    (format!("WHERE {}", clauses.join(" AND ")), args)
}

pub(super) fn escape_like(raw: &str) -> String {
    raw.to_lowercase()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub(super) fn cost_stats(conn: &Connection, workspace: &str) -> Result<(i64, u64)> {
    let cost: i64 = conn.query_row(
        "SELECT COALESCE(SUM(e.cost_usd_e6),0) FROM events e JOIN sessions s ON s.id=e.session_id WHERE s.workspace=?1",
        params![workspace], |r| r.get(0),
    )?;
    let with_cost: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT s.id) FROM sessions s JOIN events e ON e.session_id=s.id WHERE s.workspace=?1 AND e.cost_usd_e6 IS NOT NULL",
        params![workspace], |r| r.get(0),
    )?;
    Ok((cost, with_cost as u64))
}

pub(super) fn outcome_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionOutcomeRow> {
    let build_raw: Option<i64> = r.get(4)?;
    let ci_raw: Option<i64> = r.get(8)?;
    Ok(SessionOutcomeRow {
        session_id: r.get(0)?,
        test_passed: r.get(1)?,
        test_failed: r.get(2)?,
        test_skipped: r.get(3)?,
        build_ok: build_raw.map(|v| v != 0),
        lint_errors: r.get(5)?,
        revert_lines_14d: r.get(6)?,
        pr_open: r.get(7)?,
        ci_ok: ci_raw.map(|v| v != 0),
        measured_at_ms: r.get::<_, i64>(9)? as u64,
        measure_error: r.get(10)?,
    })
}

pub(super) fn feedback_row(
    r: &rusqlite::Row<'_>,
) -> rusqlite::Result<crate::feedback::types::FeedbackRecord> {
    use crate::feedback::types::{FeedbackLabel, FeedbackRecord, FeedbackScore};
    let score = r
        .get::<_, Option<i64>>(2)?
        .and_then(|v| FeedbackScore::new(v as u8));
    let label = r
        .get::<_, Option<String>>(3)?
        .and_then(|s| FeedbackLabel::from_str_opt(&s));
    Ok(FeedbackRecord {
        id: r.get(0)?,
        session_id: r.get(1)?,
        score,
        label,
        note: r.get(4)?,
        created_at_ms: r.get::<_, i64>(5)? as u64,
    })
}

pub(super) fn day_label(day_idx: u64) -> &'static str {
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][((day_idx + 4) % 7) as usize]
}

pub(super) fn sessions_by_day_7(
    conn: &Connection,
    workspace: &str,
    now: u64,
) -> Result<Vec<(String, u64)>> {
    let week_ago = now.saturating_sub(7 * 86_400_000);
    let mut stmt = conn
        .prepare("SELECT started_at_ms FROM sessions WHERE workspace=?1 AND started_at_ms>=?2")?;
    let days: Vec<u64> = stmt
        .query_map(params![workspace, week_ago as i64], |r| r.get::<_, i64>(0))?
        .filter_map(|r| r.ok())
        .map(|v| v as u64 / 86_400_000)
        .collect();
    let today = now / 86_400_000;
    Ok((0u64..7)
        .map(|i| {
            let d = today.saturating_sub(6 - i);
            (
                day_label(d).to_string(),
                days.iter().filter(|&&x| x == d).count() as u64,
            )
        })
        .collect())
}

pub(super) fn recent_sessions_3(
    conn: &Connection,
    workspace: &str,
) -> Result<Vec<(SessionRecord, u64)>> {
    let sql = "SELECT s.id,s.agent,s.model,s.workspace,s.started_at_ms,s.ended_at_ms,\
               s.status,s.trace_path,s.start_commit,s.end_commit,s.branch,s.dirty_start,\
               s.dirty_end,s.repo_binding_source,s.prompt_fingerprint,s.parent_session_id,\
               s.agent_version,s.os,s.arch,s.repo_file_count,s.repo_total_loc,\
               COUNT(e.id) FROM sessions s \
               LEFT JOIN events e ON e.session_id=s.id WHERE s.workspace=?1 \
               GROUP BY s.id ORDER BY s.started_at_ms DESC LIMIT 3";
    let mut stmt = conn.prepare(sql)?;
    let out: Vec<(SessionRecord, u64)> = stmt
        .query_map(params![workspace], |r| {
            let st: String = r.get(6)?;
            Ok((
                SessionRecord {
                    id: r.get(0)?,
                    agent: r.get(1)?,
                    model: r.get(2)?,
                    workspace: r.get(3)?,
                    started_at_ms: r.get::<_, i64>(4)? as u64,
                    ended_at_ms: r.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                    status: status_from_str(&st),
                    trace_path: r.get(7)?,
                    start_commit: r.get(8)?,
                    end_commit: r.get(9)?,
                    branch: r.get(10)?,
                    dirty_start: r.get::<_, Option<i64>>(11)?.map(i64_to_bool),
                    dirty_end: r.get::<_, Option<i64>>(12)?.map(i64_to_bool),
                    repo_binding_source: empty_to_none(r.get::<_, String>(13)?),
                    prompt_fingerprint: r.get(14)?,
                    parent_session_id: r.get(15)?,
                    agent_version: r.get(16)?,
                    os: r.get(17)?,
                    arch: r.get(18)?,
                    repo_file_count: r.get::<_, Option<i64>>(19)?.map(|v| v as u32),
                    repo_total_loc: r.get::<_, Option<i64>>(20)?.map(|v| v as u64),
                },
                r.get::<_, i64>(21)? as u64,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(out)
}

pub(super) fn top_tools_5(conn: &Connection, workspace: &str) -> Result<Vec<(String, u64)>> {
    let mut stmt = conn.prepare(
        "SELECT tool, COUNT(*) FROM events e JOIN sessions s ON s.id=e.session_id \
         WHERE s.workspace=?1 AND tool IS NOT NULL GROUP BY tool ORDER BY COUNT(*) DESC LIMIT 5",
    )?;
    let out: Vec<(String, u64)> = stmt
        .query_map(params![workspace], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(out)
}

pub(super) fn trace_span_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TraceSpanRecord> {
    let kind: String = row.get(4)?;
    let payload: String = row.get(18)?;
    Ok(TraceSpanRecord {
        span_id: row.get(0)?,
        trace_id: row.get(1)?,
        parent_span_id: row.get(2)?,
        session_id: row.get(3)?,
        kind: TraceSpanKind::parse(&kind),
        name: row.get(5)?,
        status: row.get(6)?,
        started_at_ms: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
        ended_at_ms: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
        duration_ms: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
        model: row.get(10)?,
        tool: row.get(11)?,
        tokens_in: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
        tokens_out: row.get::<_, Option<i64>>(13)?.map(|v| v as u32),
        reasoning_tokens: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
        cost_usd_e6: row.get(15)?,
        context_used_tokens: row.get::<_, Option<i64>>(16)?.map(|v| v as u32),
        context_max_tokens: row.get::<_, Option<i64>>(17)?.map(|v| v as u32),
        payload: serde_json::from_str(&payload).unwrap_or_default(),
    })
}

pub(super) fn status_from_str(s: &str) -> SessionStatus {
    match s {
        "Running" => SessionStatus::Running,
        "Waiting" => SessionStatus::Waiting,
        "Idle" => SessionStatus::Idle,
        _ => SessionStatus::Done,
    }
}

pub(super) fn kind_from_str(s: &str) -> EventKind {
    match s {
        "ToolCall" => EventKind::ToolCall,
        "ToolResult" => EventKind::ToolResult,
        "Message" => EventKind::Message,
        "Error" => EventKind::Error,
        "Cost" => EventKind::Cost,
        "Hook" => EventKind::Hook,
        "Lifecycle" => EventKind::Lifecycle,
        _ => EventKind::Hook,
    }
}

pub(super) fn source_from_str(s: &str) -> EventSource {
    match s {
        "Tail" => EventSource::Tail,
        "Hook" => EventSource::Hook,
        _ => EventSource::Proxy,
    }
}

pub(super) fn bool_to_i64(v: bool) -> i64 {
    if v { 1 } else { 0 }
}

pub(super) fn i64_to_bool(v: i64) -> bool {
    v != 0
}

pub(super) fn empty_to_none(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}
