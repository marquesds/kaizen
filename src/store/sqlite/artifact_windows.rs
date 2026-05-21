use super::*;

impl Store {
    /// Distinct `(session_id, path)` for sessions with activity in the time window.
    pub fn files_touched_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ft.session_id, ft.path
             FROM files_touched ft
             JOIN sessions s ON s.id = ft.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = ft.session_id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND ss.started_at_ms >= ?2 AND ss.started_at_ms <= ?3)
                   )
               )
             ORDER BY ft.session_id, ft.path",
        )?;
        let out: Vec<(String, String)> = stmt
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
        Ok(out)
    }

    /// Distinct skill slugs referenced in `skills_used` for a workspace since `since_ms`
    /// (any session with an indexed skill row; join events optional — use row existence).
    pub fn skills_used_since(&self, workspace: &str, since_ms: u64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT su.skill
             FROM skills_used su
             JOIN sessions s ON s.id = su.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = su.session_id
                   AND (e.ts_ms >= ?2 OR (e.ts_ms < ?3 AND ss.started_at_ms >= ?2))
               )
             ORDER BY su.skill",
        )?;
        let out: Vec<String> = stmt
            .query_map(
                params![workspace, since_ms as i64, SYNTHETIC_TS_CEILING_MS],
                |r| r.get::<_, String>(0),
            )?
            .filter_map(|r| r.ok())
            .filter(|s: &String| crate::store::event_index::is_valid_slug(s))
            .collect();
        Ok(out)
    }

    /// Distinct `(session_id, skill)` for sessions with activity in the time window.
    pub fn skills_used_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT su.session_id, su.skill
             FROM skills_used su
             JOIN sessions s ON s.id = su.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = su.session_id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND ss.started_at_ms >= ?2 AND ss.started_at_ms <= ?3)
                   )
               )
             ORDER BY su.session_id, su.skill",
        )?;
        let out: Vec<(String, String)> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?
            .filter_map(|r| r.ok())
            .filter(|(_, skill): &(String, String)| crate::store::event_index::is_valid_slug(skill))
            .collect();
        Ok(out)
    }

    /// Distinct rule stems referenced in `rules_used` for a workspace since `since_ms`.
    pub fn rules_used_since(&self, workspace: &str, since_ms: u64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ru.rule
             FROM rules_used ru
             JOIN sessions s ON s.id = ru.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = ru.session_id
                   AND (e.ts_ms >= ?2 OR (e.ts_ms < ?3 AND ss.started_at_ms >= ?2))
               )
             ORDER BY ru.rule",
        )?;
        let out: Vec<String> = stmt
            .query_map(
                params![workspace, since_ms as i64, SYNTHETIC_TS_CEILING_MS],
                |r| r.get::<_, String>(0),
            )?
            .filter_map(|r| r.ok())
            .filter(|s: &String| crate::store::event_index::is_valid_slug(s))
            .collect();
        Ok(out)
    }

    /// Distinct `(session_id, rule)` for sessions with activity in the time window.
    pub fn rules_used_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ru.session_id, ru.rule
             FROM rules_used ru
             JOIN sessions s ON s.id = ru.session_id
             WHERE s.workspace = ?1
               AND EXISTS (
                 SELECT 1 FROM events e
                 JOIN sessions ss ON ss.id = e.session_id
                 WHERE e.session_id = ru.session_id
                   AND (
                     (e.ts_ms >= ?2 AND e.ts_ms <= ?3)
                     OR (e.ts_ms < ?4 AND ss.started_at_ms >= ?2 AND ss.started_at_ms <= ?3)
                   )
               )
             ORDER BY ru.session_id, ru.rule",
        )?;
        let out: Vec<(String, String)> = stmt
            .query_map(
                params![
                    workspace,
                    start_ms as i64,
                    end_ms as i64,
                    SYNTHETIC_TS_CEILING_MS,
                ],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?
            .filter_map(|r| r.ok())
            .filter(|(_, rule): &(String, String)| crate::store::event_index::is_valid_slug(rule))
            .collect();
        Ok(out)
    }
}
