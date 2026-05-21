use super::*;

impl Store {
    /// Per-session sum of `cost_usd_e6` for events in the window (missing costs treated as 0).
    pub fn session_costs_usd_e6_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<HashMap<String, i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.session_id, SUM(COALESCE(e.cost_usd_e6, 0))
             FROM events e
             JOIN sessions s ON s.id = e.session_id
             WHERE s.workspace = ?1
               AND (
                 (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                 OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3)
               )
             GROUP BY e.session_id",
        )?;
        let rows: Vec<(String, i64)> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows.into_iter().collect())
    }
}
