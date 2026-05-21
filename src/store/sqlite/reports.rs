use super::rows::*;
use super::*;

impl Store {
    pub fn summary_stats(&self, workspace: &str) -> Result<SummaryStats> {
        let session_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE workspace = ?1",
            params![workspace],
            |r| r.get(0),
        )?;

        let total_cost: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(e.cost_usd_e6), 0) FROM events e
             JOIN sessions s ON s.id = e.session_id WHERE s.workspace = ?1",
            params![workspace],
            |r| r.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT agent, COUNT(*) FROM sessions WHERE workspace = ?1 GROUP BY agent ORDER BY COUNT(*) DESC",
        )?;
        let by_agent: Vec<(String, u64)> = stmt
            .query_map(params![workspace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(model, 'unknown'), COUNT(*) FROM sessions WHERE workspace = ?1 GROUP BY model ORDER BY COUNT(*) DESC",
        )?;
        let by_model: Vec<(String, u64)> = stmt
            .query_map(params![workspace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut stmt = self.conn.prepare(
            "SELECT tool, COUNT(*) FROM events e JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1 AND tool IS NOT NULL
             GROUP BY tool ORDER BY COUNT(*) DESC LIMIT 10",
        )?;
        let top_tools: Vec<(String, u64)> = stmt
            .query_map(params![workspace], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(SummaryStats {
            session_count: session_count as u64,
            total_cost_usd_e6: total_cost,
            by_agent,
            by_model,
            top_tools,
        })
    }

    /// Workspace activity dashboard — feeds `cmd_insights`.
    pub fn insights(&self, workspace: &str) -> Result<InsightsStats> {
        let (total_cost_usd_e6, sessions_with_cost) = cost_stats(&self.conn, workspace)?;
        Ok(InsightsStats {
            total_sessions: count_q(
                &self.conn,
                "SELECT COUNT(*) FROM sessions WHERE workspace=?1",
                workspace,
            )?,
            running_sessions: count_q(
                &self.conn,
                "SELECT COUNT(*) FROM sessions WHERE workspace=?1 AND status='Running'",
                workspace,
            )?,
            total_events: count_q(
                &self.conn,
                "SELECT COUNT(*) FROM events e JOIN sessions s ON s.id=e.session_id WHERE s.workspace=?1",
                workspace,
            )?,
            sessions_by_day: sessions_by_day_7(&self.conn, workspace, now_ms())?,
            recent: recent_sessions_3(&self.conn, workspace)?,
            top_tools: top_tools_5(&self.conn, workspace)?,
            total_cost_usd_e6,
            sessions_with_cost,
        })
    }
}
