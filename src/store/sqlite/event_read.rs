use super::rows::*;
use super::*;

impl Store {
    pub fn list_events_for_session(&self, session_id: &str) -> Result<Vec<Event>> {
        self.list_events_page(session_id, 0, i64::MAX as usize)
    }

    pub fn get_event(&self, session_id: &str, seq: u64) -> Result<Option<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, seq, ts_ms, COALESCE(ts_exact, 0), kind, source, tool, tool_call_id,
                    tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload,
                    stop_reason, latency_ms, ttft_ms, retry_count,
                    context_used_tokens, context_max_tokens,
                    cache_creation_tokens, cache_read_tokens, system_prompt_tokens
             FROM events WHERE session_id = ?1 AND seq = ?2",
        )?;
        stmt.query_row(params![session_id, seq as i64], event_row)
            .optional()
            .map_err(Into::into)
    }

    pub fn search_tool_events(
        &self,
        workspace: &str,
        tool: &str,
        since_ms: Option<u64>,
        agent: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, Event)>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.session_id, e.seq, e.ts_ms, COALESCE(e.ts_exact, 0), e.kind, e.source, e.tool, e.tool_call_id,
                    e.tokens_in, e.tokens_out, e.reasoning_tokens, e.cost_usd_e6, e.payload,
                    e.stop_reason, e.latency_ms, e.ttft_ms, e.retry_count,
                    e.context_used_tokens, e.context_max_tokens,
                    e.cache_creation_tokens, e.cache_read_tokens, e.system_prompt_tokens,
                    s.agent
             FROM events e JOIN sessions s ON s.id = e.session_id
             WHERE e.tool = ?2
               AND (s.workspace = ?1 OR NOT EXISTS (SELECT 1 FROM sessions WHERE workspace = ?1))
               AND (?3 IS NULL OR e.ts_ms >= ?3)
               AND (?4 IS NULL OR s.agent = ?4)
             ORDER BY e.ts_ms DESC, e.session_id ASC, e.seq ASC
             LIMIT ?5",
        )?;
        let since = since_ms.map(|v| v as i64);
        let rows = stmt.query_map(
            params![workspace, tool, since, agent, limit as i64],
            search_tool_event_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn workspace_events(&self, workspace: &str) -> Result<Vec<(SessionRecord, Event)>> {
        let mut out = Vec::new();
        for session in self.list_sessions(workspace)? {
            for event in self.list_events_for_session(&session.id)? {
                out.push((session.clone(), event));
            }
        }
        out.sort_by(|a, b| {
            (a.1.ts_ms, &a.1.session_id, a.1.seq).cmp(&(b.1.ts_ms, &b.1.session_id, b.1.seq))
        });
        Ok(out)
    }

    pub fn list_events_page(
        &self,
        session_id: &str,
        after_seq: u64,
        limit: usize,
    ) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, seq, ts_ms, COALESCE(ts_exact, 0), kind, source, tool, tool_call_id,
                    tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload,
                    stop_reason, latency_ms, ttft_ms, retry_count,
                    context_used_tokens, context_max_tokens,
                    cache_creation_tokens, cache_read_tokens, system_prompt_tokens
             FROM events
             WHERE session_id = ?1 AND seq >= ?2
             ORDER BY seq ASC LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![
                session_id,
                after_seq as i64,
                limit.min(i64::MAX as usize) as i64
            ],
            event_row,
        )?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn last_event_seq_for_session(&self, session_id: &str) -> Result<Option<u64>> {
        let seq = self
            .conn
            .query_row(
                "SELECT MAX(seq) FROM events WHERE session_id = ?1",
                params![session_id],
                |r| r.get::<_, Option<i64>>(0),
            )?
            .map(|v| v as u64);
        Ok(seq)
    }
}
