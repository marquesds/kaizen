use super::rows::*;
use super::*;

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
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
            let paths_json: String = row.get(8)?;
            Ok(ToolSpanView {
                span_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                tool: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "unknown".into()),
                status: row.get(2)?,
                lead_time_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                cost_usd_e6: row.get(7)?,
                paths: serde_json::from_str(&paths_json).unwrap_or_default(),
                parent_span_id: row.get(9)?,
                depth: row.get::<_, Option<i64>>(10)?.unwrap_or(0) as u32,
                subtree_cost_usd_e6: row.get(11)?,
                subtree_token_count: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub fn session_span_tree(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::store::span_tree::SpanNode>> {
        let last_event_seq = self.last_event_seq_for_session(session_id)?;
        if let Some(entry) = self.span_tree_cache.borrow().as_ref()
            && entry.session_id == session_id
            && entry.last_event_seq == last_event_seq
        {
            return Ok(entry.nodes.clone());
        }
        let mut stmt = self.conn.prepare(
            "SELECT span_id, tool, status, lead_time_ms, tokens_in, tokens_out,
                    reasoning_tokens, cost_usd_e6, paths_json,
                    parent_span_id, depth, subtree_cost_usd_e6, subtree_token_count
             FROM tool_spans
             WHERE session_id = ?1
             ORDER BY depth ASC, started_at_ms ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let paths_json: String = row.get(8)?;
            Ok(crate::metrics::types::ToolSpanView {
                span_id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                tool: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "unknown".into()),
                status: row.get(2)?,
                lead_time_ms: row.get::<_, Option<i64>>(3)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                cost_usd_e6: row.get(7)?,
                paths: serde_json::from_str(&paths_json).unwrap_or_default(),
                parent_span_id: row.get(9)?,
                depth: row.get::<_, Option<i64>>(10)?.unwrap_or(0) as u32,
                subtree_cost_usd_e6: row.get(11)?,
                subtree_token_count: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
            })
        })?;
        let spans: Vec<_> = rows.filter_map(|r| r.ok()).collect();
        let nodes = crate::store::span_tree::build_tree(spans);
        *self.span_tree_cache.borrow_mut() = Some(SpanTreeCacheEntry {
            session_id: session_id.to_string(),
            last_event_seq,
            nodes: nodes.clone(),
        });
        Ok(nodes)
    }

    pub fn tool_spans_for_session(&self, session_id: &str) -> Result<Vec<ToolSpanSyncRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT span_id, session_id, tool, tool_call_id, status, started_at_ms, ended_at_ms, lead_time_ms,
                    tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json
             FROM tool_spans WHERE session_id = ?1 ORDER BY started_at_ms ASC, span_id ASC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let paths_json: String = row.get(12)?;
            Ok(ToolSpanSyncRow {
                span_id: row.get(0)?,
                session_id: row.get(1)?,
                tool: row.get(2)?,
                tool_call_id: row.get(3)?,
                status: row.get(4)?,
                started_at_ms: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                ended_at_ms: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                lead_time_ms: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                tokens_in: row.get::<_, Option<i64>>(8)?.map(|v| v as u32),
                tokens_out: row.get::<_, Option<i64>>(9)?.map(|v| v as u32),
                reasoning_tokens: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
                cost_usd_e6: row.get(11)?,
                paths: serde_json::from_str(&paths_json).unwrap_or_default(),
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }
}
