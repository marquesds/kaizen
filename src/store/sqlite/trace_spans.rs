use super::rows::*;
use super::*;

impl Store {
    pub fn upsert_trace_span(&self, span: &TraceSpanRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO trace_spans (
                span_id, trace_id, parent_span_id, session_id, kind, name, status,
                started_at_ms, ended_at_ms, duration_ms, model, tool, tokens_in, tokens_out,
                reasoning_tokens, cost_usd_e6, context_used_tokens, context_max_tokens, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
             ON CONFLICT(span_id) DO UPDATE SET
                trace_id=excluded.trace_id, parent_span_id=excluded.parent_span_id,
                session_id=excluded.session_id, kind=excluded.kind, name=excluded.name,
                status=excluded.status, started_at_ms=excluded.started_at_ms,
                ended_at_ms=excluded.ended_at_ms, duration_ms=excluded.duration_ms,
                model=excluded.model, tool=excluded.tool, tokens_in=excluded.tokens_in,
                tokens_out=excluded.tokens_out, reasoning_tokens=excluded.reasoning_tokens,
                cost_usd_e6=excluded.cost_usd_e6,
                context_used_tokens=excluded.context_used_tokens,
                context_max_tokens=excluded.context_max_tokens, payload=excluded.payload",
            params![
                span.span_id.as_str(),
                span.trace_id.as_str(),
                span.parent_span_id.as_deref(),
                span.session_id.as_str(),
                span.kind.as_str(),
                span.name.as_str(),
                span.status.as_str(),
                span.started_at_ms.map(|v| v as i64),
                span.ended_at_ms.map(|v| v as i64),
                span.duration_ms.map(|v| v as i64),
                span.model.as_deref(),
                span.tool.as_deref(),
                span.tokens_in.map(|v| v as i64),
                span.tokens_out.map(|v| v as i64),
                span.reasoning_tokens.map(|v| v as i64),
                span.cost_usd_e6,
                span.context_used_tokens.map(|v| v as i64),
                span.context_max_tokens.map(|v| v as i64),
                serde_json::to_string(&span.payload)?,
            ],
        )?;
        Ok(())
    }

    pub fn trace_spans_for_session(&self, session_id: &str) -> Result<Vec<TraceSpanRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT span_id, trace_id, parent_span_id, session_id, kind, name, status,
                    started_at_ms, ended_at_ms, duration_ms, model, tool, tokens_in, tokens_out,
                    reasoning_tokens, cost_usd_e6, context_used_tokens, context_max_tokens, payload
             FROM trace_spans WHERE session_id = ?1
             ORDER BY COALESCE(started_at_ms, ended_at_ms, 0), span_id",
        )?;
        let rows = stmt.query_map(params![session_id], trace_span_from_row)?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub(crate) fn capture_quality_rows(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<CaptureQualityRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.source,
                    e.tokens_in IS NOT NULL OR e.tokens_out IS NOT NULL OR e.reasoning_tokens IS NOT NULL,
                    e.latency_ms IS NOT NULL OR e.ttft_ms IS NOT NULL,
                    e.context_used_tokens IS NOT NULL AND e.context_max_tokens IS NOT NULL
             FROM events e JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1 AND e.ts_ms >= ?2 AND e.ts_ms <= ?3",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
            Ok(CaptureQualityRow {
                source: row.get(0)?,
                has_tokens: row.get::<_, i64>(1)? != 0,
                has_latency: row.get::<_, i64>(2)? != 0,
                has_context: row.get::<_, i64>(3)? != 0,
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }

    pub(crate) fn trace_span_quality_rows(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<TraceSpanQualityRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT ts.kind,
                    ts.parent_span_id IS NOT NULL
                    AND parent.span_id IS NULL
                    AND ts.kind NOT IN ('session', 'agent')
             FROM trace_spans ts
             JOIN sessions s ON s.id = ts.session_id
             LEFT JOIN trace_spans parent ON parent.span_id = ts.parent_span_id
             WHERE s.workspace = ?1
               AND COALESCE(ts.started_at_ms, ts.ended_at_ms, 0) >= ?2
               AND COALESCE(ts.started_at_ms, ts.ended_at_ms, 0) <= ?3",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |row| {
            Ok(TraceSpanQualityRow {
                kind: row.get(0)?,
                is_orphan: row.get::<_, i64>(1)? != 0,
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }
}
