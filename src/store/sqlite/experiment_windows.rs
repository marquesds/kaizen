use super::rows::*;
use super::*;
impl Store {
    /// Events in `[start_ms, end_ms]` for a workspace, with session metadata per row.
    pub fn retro_events_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(SessionRecord, Event)>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.session_id, e.seq, e.ts_ms, COALESCE(e.ts_exact, 0), e.kind, e.source, e.tool, e.tool_call_id,
                    e.tokens_in, e.tokens_out, e.reasoning_tokens, e.cost_usd_e6, e.payload,
                    s.id, s.agent, s.model, s.workspace, s.started_at_ms, s.ended_at_ms, s.status, s.trace_path,
                    s.start_commit, s.end_commit, s.branch, s.dirty_start, s.dirty_end, s.repo_binding_source,
                    s.prompt_fingerprint, s.parent_session_id, s.agent_version, s.os, s.arch,
                    s.repo_file_count, s.repo_total_loc,
                    e.stop_reason, e.latency_ms, e.ttft_ms, e.retry_count,
                    e.context_used_tokens, e.context_max_tokens,
                    e.cache_creation_tokens, e.cache_read_tokens, e.system_prompt_tokens
             FROM events e
             JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1
               AND (
                 (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                 OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3)
               )
             ORDER BY e.ts_ms ASC, e.session_id ASC, e.seq ASC",
        )?;
        let rows = stmt.query_map(
            params![
                workspace,
                start_ms as i64,
                end_ms as i64,
                SYNTHETIC_TS_CEILING_MS,
            ],
            |row| {
                let payload_str: String = row.get(12)?;
                let status_str: String = row.get(19)?;
                Ok((
                    SessionRecord {
                        id: row.get(13)?,
                        agent: row.get(14)?,
                        model: row.get(15)?,
                        workspace: row.get(16)?,
                        started_at_ms: row.get::<_, i64>(17)? as u64,
                        ended_at_ms: row.get::<_, Option<i64>>(18)?.map(|v| v as u64),
                        status: status_from_str(&status_str),
                        trace_path: row.get(20)?,
                        start_commit: row.get(21)?,
                        end_commit: row.get(22)?,
                        branch: row.get(23)?,
                        dirty_start: row.get::<_, Option<i64>>(24)?.map(i64_to_bool),
                        dirty_end: row.get::<_, Option<i64>>(25)?.map(i64_to_bool),
                        repo_binding_source: empty_to_none(row.get::<_, String>(26)?),
                        prompt_fingerprint: row.get(27)?,
                        parent_session_id: row.get(28)?,
                        agent_version: row.get(29)?,
                        os: row.get(30)?,
                        arch: row.get(31)?,
                        repo_file_count: row.get::<_, Option<i64>>(32)?.map(|v| v as u32),
                        repo_total_loc: row.get::<_, Option<i64>>(33)?.map(|v| v as u64),
                    },
                    Event {
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
                        payload: serde_json::from_str(&payload_str)
                            .unwrap_or(serde_json::Value::Null),
                        stop_reason: row.get(34)?,
                        latency_ms: row.get::<_, Option<i64>>(35)?.map(|v| v as u32),
                        ttft_ms: row.get::<_, Option<i64>>(36)?.map(|v| v as u32),
                        retry_count: row.get::<_, Option<i64>>(37)?.map(|v| v as u16),
                        context_used_tokens: row.get::<_, Option<i64>>(38)?.map(|v| v as u32),
                        context_max_tokens: row.get::<_, Option<i64>>(39)?.map(|v| v as u32),
                        cache_creation_tokens: row.get::<_, Option<i64>>(40)?.map(|v| v as u32),
                        cache_read_tokens: row.get::<_, Option<i64>>(41)?.map(|v| v as u32),
                        system_prompt_tokens: row.get::<_, Option<i64>>(42)?.map(|v| v as u32),
                    },
                ))
            },
        )?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
    pub fn experiment_metric_values_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
        metric: crate::experiment::types::Metric,
    ) -> Result<Vec<(SessionRecord, f64)>> {
        use crate::experiment::types::Metric;
        let session_cols = "s.id, s.agent, s.model, s.workspace, s.started_at_ms, s.ended_at_ms,
            s.status, s.trace_path, s.start_commit, s.end_commit, s.branch, s.dirty_start,
            s.dirty_end, s.repo_binding_source, s.prompt_fingerprint, s.parent_session_id,
            s.agent_version, s.os, s.arch, s.repo_file_count, s.repo_total_loc";
        let window = "s.workspace = ?1 AND ((e.ts_ms >= ?2 AND e.ts_ms <= ?3)
            OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3))";
        let sql = match metric {
            Metric::TokensPerSession => format!(
                "SELECT {session_cols},
                    SUM(COALESCE(e.tokens_in,0)+COALESCE(e.tokens_out,0)+COALESCE(e.reasoning_tokens,0)) AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::CostPerSession => format!(
                "SELECT {session_cols}, SUM(COALESCE(e.cost_usd_e6,0)) / 1000000.0 AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::SuccessRate => format!(
                "SELECT {session_cols},
                    CASE WHEN SUM(CASE WHEN e.kind='Error' THEN 1 ELSE 0 END) > 0 THEN 0.0 ELSE 1.0 END AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::DurationMinutes => format!(
                "SELECT {session_cols},
                    (s.ended_at_ms - s.started_at_ms) / 60000.0 AS value
                 FROM sessions s
                 WHERE s.workspace = ?1
                   AND s.ended_at_ms IS NOT NULL
                   AND EXISTS (
                     SELECT 1 FROM events e
                     WHERE e.session_id = s.id
                       AND ((e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                         OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3))
                   )"
            ),
            Metric::FilesPerSession => format!(
                "SELECT {session_cols}, COUNT(DISTINCT ft.path) AS value
                 FROM sessions s
                 JOIN events e ON e.session_id = s.id
                 LEFT JOIN files_touched ft ON ft.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id"
            ),
            Metric::SuccessRateByPrompt => format!(
                "SELECT {session_cols},
                    1.0 - (MIN(
                      SUM(CASE WHEN e.kind='Error' THEN 1 ELSE 0 END),
                      SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END)
                    ) * 1.0 / SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END)) AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id
                 HAVING SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END) > 0"
            ),
            Metric::CostByPrompt => format!(
                "SELECT {session_cols},
                    SUM(COALESCE(e.cost_usd_e6,0)) / 1000000.0 /
                    SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END) AS value
                 FROM sessions s JOIN events e ON e.session_id = s.id
                 WHERE {window}
                 GROUP BY s.id
                 HAVING SUM(CASE WHEN e.kind='Message' THEN 1 ELSE 0 END) > 0"
            ),
            Metric::ToolLoops => format!(
                "WITH calls AS (
                   SELECT e.session_id, e.tool,
                     LAG(e.tool) OVER (PARTITION BY e.session_id ORDER BY e.ts_ms, e.seq) AS prev_tool
                   FROM events e JOIN sessions s ON s.id = e.session_id
                   WHERE {window} AND e.kind='ToolCall' AND e.tool IS NOT NULL
                 )
                 SELECT {session_cols},
                    SUM(CASE WHEN calls.tool = calls.prev_tool THEN 1 ELSE 0 END) AS value
                 FROM sessions s JOIN calls ON calls.session_id = s.id
                 GROUP BY s.id"
            ),
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                workspace,
                start_ms as i64,
                end_ms as i64,
                SYNTHETIC_TS_CEILING_MS,
            ],
            |row| Ok((session_row(row)?, row.get::<_, f64>(21)?)),
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }
}
