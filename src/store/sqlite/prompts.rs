use super::*;

impl Store {
    pub fn upsert_prompt_snapshot(&self, snap: &crate::prompt::PromptSnapshot) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO prompt_snapshots
             (fingerprint, captured_at_ms, files_json, total_bytes)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                snap.fingerprint,
                snap.captured_at_ms as i64,
                snap.files_json,
                snap.total_bytes as i64
            ],
        )?;
        Ok(())
    }

    pub fn get_prompt_snapshot(
        &self,
        fingerprint: &str,
    ) -> Result<Option<crate::prompt::PromptSnapshot>> {
        self.conn
            .query_row(
                "SELECT fingerprint, captured_at_ms, files_json, total_bytes
                 FROM prompt_snapshots WHERE fingerprint = ?1",
                params![fingerprint],
                |r| {
                    Ok(crate::prompt::PromptSnapshot {
                        fingerprint: r.get(0)?,
                        captured_at_ms: r.get::<_, i64>(1)? as u64,
                        files_json: r.get(2)?,
                        total_bytes: r.get::<_, i64>(3)? as u64,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_prompt_snapshots(&self) -> Result<Vec<crate::prompt::PromptSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT fingerprint, captured_at_ms, files_json, total_bytes
             FROM prompt_snapshots ORDER BY captured_at_ms DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(crate::prompt::PromptSnapshot {
                fingerprint: r.get(0)?,
                captured_at_ms: r.get::<_, i64>(1)? as u64,
                files_json: r.get(2)?,
                total_bytes: r.get::<_, i64>(3)? as u64,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Sessions with a non-null prompt_fingerprint in the given window.
    pub fn sessions_with_prompt_fingerprint(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, prompt_fingerprint FROM sessions
             WHERE workspace = ?1
               AND started_at_ms >= ?2 AND started_at_ms < ?3
               AND prompt_fingerprint IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
