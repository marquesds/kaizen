use super::*;

impl Store {
    /// Sync-shaped tool spans whose session falls in `[start_ms, end_ms]`. Mirrors
    /// `retro_events_in_window` for the spans table so `kaizen telemetry push` can ship
    /// `IngestExportBatch::ToolSpans` next to the events batch. Window matches on
    /// `started_at_ms` first, falling back to `ended_at_ms` for spans that never started a
    /// timer (status-only rows). Workspace filter joins through `sessions.workspace`.
    pub fn tool_spans_sync_rows_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<ToolSpanSyncRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT span_id, session_id, tool, tool_call_id, status, started_at_ms, ended_at_ms,
                    lead_time_ms, tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json
             FROM (
                 SELECT ts.span_id, ts.session_id, ts.tool, ts.tool_call_id, ts.status,
                        ts.started_at_ms, ts.ended_at_ms, ts.lead_time_ms, ts.tokens_in,
                        ts.tokens_out, ts.reasoning_tokens, ts.cost_usd_e6, ts.paths_json,
                        ts.started_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms IS NOT NULL
                   AND ts.started_at_ms >= ?2
                   AND ts.started_at_ms <= ?3
                 UNION ALL
                 SELECT ts.span_id, ts.session_id, ts.tool, ts.tool_call_id, ts.status,
                        ts.started_at_ms, ts.ended_at_ms, ts.lead_time_ms, ts.tokens_in,
                        ts.tokens_out, ts.reasoning_tokens, ts.cost_usd_e6, ts.paths_json,
                        ts.ended_at_ms AS sort_ms
                 FROM tool_spans ts
                 JOIN sessions s ON s.id = ts.session_id
                 WHERE s.workspace = ?1
                   AND ts.started_at_ms IS NULL
                   AND ts.ended_at_ms IS NOT NULL
                   AND ts.ended_at_ms >= ?2
                   AND ts.ended_at_ms <= ?3
             )
             ORDER BY sort_ms ASC, span_id ASC",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
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
