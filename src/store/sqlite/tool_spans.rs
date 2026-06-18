use super::rows::*;
use super::*;
mod view_row;
use view_row::tool_span_view_row;

const SESSION_SPAN_TREE_SQL: &str =
    "SELECT span_id, tool, status, lead_time_ms, tokens_in, tokens_out,
            reasoning_tokens, cost_usd_e6, paths_json,
            parent_span_id, depth, subtree_cost_usd_e6, subtree_token_count
     FROM tool_spans
     WHERE session_id = ?1
     ORDER BY depth ASC, started_at_ms ASC, span_id ASC
     LIMIT ?2";

const SESSION_TOOL_SPANS_SQL: &str =
    "SELECT span_id, session_id, tool, tool_call_id, status, started_at_ms, ended_at_ms, lead_time_ms,
            tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json
     FROM tool_spans WHERE session_id = ?1 ORDER BY started_at_ms ASC, span_id ASC
     LIMIT ?2";

impl Store {
    pub fn tool_rank_rows_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<RankedTool>> {
        let mut stmt = self.conn.prepare(TOOL_RANK_ROWS_SQL)?;
        let rows = stmt.query_map(
            params![workspace, start_ms as i64, end_ms as i64],
            ranked_tool_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn tool_spans_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<ToolSpanView>> {
        let mut stmt = self.conn.prepare(
            "SELECT span_id, tool, status, lead_time_ms, tokens_in, tokens_out,
                    reasoning_tokens, cost_usd_e6, paths_json,
                    parent_span_id, depth, subtree_cost_usd_e6, subtree_token_count
             FROM (
                 SELECT ts.span_id, ts.tool, ts.status, ts.lead_time_ms,
                        ts.tokens_in, ts.tokens_out, ts.reasoning_tokens,
                        ts.cost_usd_e6, ts.paths_json, ts.parent_span_id,
                        ts.depth, ts.subtree_cost_usd_e6, ts.subtree_token_count,
                        ts.started_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms >= ?2
                   AND ts.started_at_ms <= ?3
                 UNION ALL
                 SELECT ts.span_id, ts.tool, ts.status, ts.lead_time_ms,
                        ts.tokens_in, ts.tokens_out, ts.reasoning_tokens,
                        ts.cost_usd_e6, ts.paths_json, ts.parent_span_id,
                        ts.depth, ts.subtree_cost_usd_e6, ts.subtree_token_count,
                        ts.ended_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms IS NULL
                   AND ts.ended_at_ms >= ?2
                   AND ts.ended_at_ms <= ?3
             )
             ORDER BY sort_ms DESC",
        )?;
        let rows = stmt.query_map(
            params![workspace, start_ms as i64, end_ms as i64],
            tool_span_view_row,
        )?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn session_span_tree(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::store::span_tree::SpanNode>> {
        let last_event_seq = self.last_event_seq_for_session(session_id)?;
        if let Some(nodes) = cached_span_tree(self, session_id, last_event_seq) {
            return Ok(nodes);
        }
        let nodes = query_session_span_tree(self, session_id, usize::MAX)?;
        cache_span_tree(self, session_id, last_event_seq, &nodes);
        Ok(nodes)
    }

    pub(crate) fn limited_session_span_tree(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<crate::store::span_tree::SpanNode>> {
        query_session_span_tree(self, session_id, limit)
    }

    pub fn tool_spans_for_session(&self, session_id: &str) -> Result<Vec<ToolSpanSyncRow>> {
        query_tool_spans_for_session(self, session_id, usize::MAX)
    }

    pub(crate) fn tool_spans_for_session_limited(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<ToolSpanSyncRow>> {
        query_tool_spans_for_session(self, session_id, limit)
    }
}

fn query_tool_spans_for_session(
    store: &Store,
    session_id: &str,
    limit: usize,
) -> Result<Vec<ToolSpanSyncRow>> {
    let mut stmt = store.conn.prepare(SESSION_TOOL_SPANS_SQL)?;
    let rows = stmt.query_map(params![session_id, sql_limit(limit)], tool_span_sync_row)?;
    rows.map(|row| row.map_err(Into::into)).collect()
}

fn tool_span_sync_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ToolSpanSyncRow> {
    Ok(ToolSpanSyncRow {
        span_id: row.get(0)?,
        session_id: row.get(1)?,
        tool: row.get(2)?,
        tool_call_id: row.get(3)?,
        status: row.get(4)?,
        paths: sync_paths(row)?,
        started_at_ms: sync_u64(row, 5)?,
        ended_at_ms: sync_u64(row, 6)?,
        lead_time_ms: sync_u64(row, 7)?,
        tokens_in: sync_u32(row, 8)?,
        tokens_out: sync_u32(row, 9)?,
        reasoning_tokens: sync_u32(row, 10)?,
        cost_usd_e6: row.get(11)?,
    })
}

fn sync_paths(row: &rusqlite::Row<'_>) -> rusqlite::Result<Vec<String>> {
    let paths_json: String = row.get(12)?;
    Ok(serde_json::from_str(&paths_json).unwrap_or_default())
}

fn sync_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value as u64))
}

fn sync_u32(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u32>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value as u32))
}

fn query_session_span_tree(
    store: &Store,
    session_id: &str,
    limit: usize,
) -> Result<Vec<crate::store::span_tree::SpanNode>> {
    let mut stmt = store.conn.prepare(SESSION_SPAN_TREE_SQL)?;
    let rows = stmt.query_map(params![session_id, sql_limit(limit)], tool_span_view_row)?;
    let spans = rows
        .map(|row| row.map_err(anyhow::Error::from))
        .collect::<Result<Vec<_>>>()?;
    Ok(crate::store::span_tree::build_tree(spans))
}

fn cached_span_tree(
    store: &Store,
    session_id: &str,
    last_event_seq: Option<u64>,
) -> Option<Vec<crate::store::span_tree::SpanNode>> {
    store
        .span_tree_cache
        .borrow()
        .as_ref()
        .filter(|entry| entry.session_id == session_id && entry.last_event_seq == last_event_seq)
        .map(|entry| entry.nodes.clone())
}

fn cache_span_tree(
    store: &Store,
    session_id: &str,
    last_event_seq: Option<u64>,
    nodes: &[crate::store::span_tree::SpanNode],
) {
    *store.span_tree_cache.borrow_mut() = Some(SpanTreeCacheEntry {
        session_id: session_id.to_string(),
        last_event_seq,
        nodes: nodes.to_vec(),
    });
}

fn sql_limit(limit: usize) -> i64 {
    limit.min(i64::MAX as usize) as i64
}
