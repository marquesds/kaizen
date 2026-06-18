use super::{SessionSummaryRead, TokenRead};
use crate::store::Store;
use crate::visualization::{TokenTotals, TraceSummary, derive_status};
use anyhow::Result;
use rusqlite::params;
use std::collections::HashMap;

const SUMMARIES_SQL: &str = "
WITH recent AS MATERIALIZED (
 SELECT id, agent, model, workspace, started_at_ms, ended_at_ms,
  status, trace_path, start_commit, end_commit, branch, dirty_start, dirty_end,
  repo_binding_source, prompt_fingerprint, parent_session_id, agent_version, os, arch,
  repo_file_count, repo_total_loc FROM sessions WHERE workspace = ?1
 ORDER BY started_at_ms DESC, id ASC LIMIT ?2
), rollup AS (
 SELECT e.session_id, MAX(e.ts_ms) last_event_ms, COUNT(*) event_count,
  SUM(e.kind = 'Error') error_count, SUM(e.kind = 'ToolCall') tool_call_count,
  COALESCE(SUM(e.cost_usd_e6), 0) cost_usd_e6,
  COALESCE(SUM(e.tokens_in), 0) tokens_in, COALESCE(SUM(e.tokens_out), 0) tokens_out,
  COALESCE(SUM(e.reasoning_tokens), 0) reasoning_tokens,
  COALESCE(SUM(e.cache_read_tokens), 0) cache_read_tokens,
  COALESCE(SUM(e.cache_creation_tokens), 0) cache_creation_tokens
 FROM events e JOIN recent r ON r.id = e.session_id GROUP BY e.session_id
)
SELECT r.*, a.last_event_ms, COALESCE(a.event_count, 0), COALESCE(a.error_count, 0),
 COALESCE(a.tool_call_count, 0), COALESCE(a.cost_usd_e6, 0), COALESCE(a.tokens_in, 0),
 COALESCE(a.tokens_out, 0), COALESCE(a.reasoning_tokens, 0),
 COALESCE(a.cache_read_tokens, 0), COALESCE(a.cache_creation_tokens, 0)
FROM recent r LEFT JOIN rollup a ON a.session_id = r.id
ORDER BY r.started_at_ms DESC, r.id ASC";

const TOP_TOOLS_SQL: &str = "
WITH recent AS MATERIALIZED (SELECT id FROM sessions WHERE workspace = ?1
 ORDER BY started_at_ms DESC, id ASC LIMIT ?2), counts AS (
 SELECT e.session_id, e.tool, COUNT(*) count FROM events e
 JOIN recent r ON r.id = e.session_id WHERE e.tool IS NOT NULL
 GROUP BY e.session_id, e.tool
), ranked AS (
 SELECT session_id, tool, count, ROW_NUMBER() OVER (
  PARTITION BY session_id ORDER BY count DESC, tool ASC) rank FROM counts
)
SELECT session_id, tool, count FROM ranked WHERE rank <= 5 ORDER BY session_id, rank";

impl Store {
    pub(crate) fn visualization_sessions(
        &self,
        workspace: &str,
        limit: usize,
        now_ms: u64,
    ) -> Result<Vec<TraceSummary>> {
        let mut rows = summary_rows(self, workspace, limit)?;
        let mut tools = top_tools(self, workspace, limit)?;
        rows.iter_mut()
            .for_each(|row| row.top_tools = tools.remove(&row.session.id).unwrap_or_default());
        Ok(rows.into_iter().map(|row| summary(row, now_ms)).collect())
    }
}

fn summary_rows(store: &Store, workspace: &str, limit: usize) -> Result<Vec<SessionSummaryRead>> {
    let mut statement = store.conn().prepare(SUMMARIES_SQL)?;
    let rows = statement.query_map(params![workspace, sql_limit(limit)], summary_row)?;
    rows.map(|row| row.map_err(Into::into)).collect()
}

fn top_tools(
    store: &Store,
    workspace: &str,
    limit: usize,
) -> Result<HashMap<String, Vec<(String, u64)>>> {
    let mut statement = store.conn().prepare(TOP_TOOLS_SQL)?;
    let rows = statement.query_map(params![workspace, sql_limit(limit)], tool_row)?;
    let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows.into_iter().fold(HashMap::new(), add_tool))
}

fn add_tool(
    mut tools: HashMap<String, Vec<(String, u64)>>,
    row: (String, String, u64),
) -> HashMap<String, Vec<(String, u64)>> {
    tools.entry(row.0).or_default().push((row.1, row.2));
    tools
}

fn tool_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, u64)> {
    Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as u64))
}

fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummaryRead> {
    Ok(SessionSummaryRead {
        session: super::super::rows::session_row(row)?,
        last_event_ms: optional(row, 21)?,
        event_count: value(row, 22)?,
        error_count: value(row, 23)?,
        tool_call_count: value(row, 24)?,
        cost_usd_e6: row.get(25)?,
        tokens: tokens(row)?,
        top_tools: Vec::new(),
    })
}

fn tokens(row: &rusqlite::Row<'_>) -> rusqlite::Result<TokenRead> {
    Ok(TokenRead {
        input: value(row, 26)?,
        output: value(row, 27)?,
        reasoning: value(row, 28)?,
        cache_read: value(row, 29)?,
        cache_create: value(row, 30)?,
    })
}

fn optional(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value as u64))
}

fn value(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    row.get::<_, i64>(index).map(|value| value as u64)
}

fn sql_limit(limit: usize) -> i64 {
    limit.min(i64::MAX as usize) as i64
}

fn summary(row: SessionSummaryRead, now_ms: u64) -> TraceSummary {
    let (status, status_reason) =
        derive_status(&row.session, row.last_event_ms, row.error_count, now_ms);
    TraceSummary {
        id: row.session.id,
        agent: row.session.agent,
        model: row.session.model,
        status,
        status_reason,
        started_at_ms: row.session.started_at_ms,
        ended_at_ms: row.session.ended_at_ms,
        last_event_ms: row.last_event_ms,
        event_count: row.event_count,
        error_count: row.error_count,
        tool_call_count: row.tool_call_count,
        cost_usd_e6: row.cost_usd_e6,
        tokens: token_totals(row.tokens),
        top_tools: row.top_tools,
    }
}

fn token_totals(row: TokenRead) -> TokenTotals {
    let total = row.input + row.output + row.reasoning + row.cache_read + row.cache_create;
    TokenTotals {
        input: row.input,
        output: row.output,
        reasoning: row.reasoning,
        cache_read: row.cache_read,
        cache_create: row.cache_create,
        total,
    }
}
