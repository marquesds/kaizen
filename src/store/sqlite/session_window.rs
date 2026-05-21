use super::*;

impl Store {
    /// Sessions with at least one event timestamp falling in `[start_ms, end_ms]` (same rules as retro window).
    pub fn sessions_active_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT s.id
             FROM sessions s
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 WHERE e.session_id = s.id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3)
                   )
               )",
        )?;
        let out: HashSet<String> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| r.get(0),
            )?
            .filter_map(|r| r.ok())
            .collect();
        Ok(out)
    }
}
