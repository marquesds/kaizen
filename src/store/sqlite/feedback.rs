use super::rows::*;
use super::*;

impl Store {
    pub fn upsert_feedback(&self, r: &crate::feedback::types::FeedbackRecord) -> Result<()> {
        use crate::feedback::types::FeedbackLabel;
        self.conn.execute(
            "INSERT OR REPLACE INTO session_feedback
             (id, session_id, score, label, note, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                r.id,
                r.session_id,
                r.score.as_ref().map(|s| s.0 as i64),
                r.label.as_ref().map(FeedbackLabel::to_db_str),
                r.note,
                r.created_at_ms as i64,
            ],
        )?;
        let payload = serde_json::to_string(r).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO sync_outbox (session_id, kind, payload, sent)
             VALUES (?1, 'session_feedback', ?2, 0)",
            rusqlite::params![r.session_id, payload],
        )?;
        Ok(())
    }

    pub fn list_feedback_in_window(
        &self,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<crate::feedback::types::FeedbackRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, score, label, note, created_at_ms
             FROM session_feedback
             WHERE created_at_ms >= ?1 AND created_at_ms < ?2
             ORDER BY created_at_ms ASC",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![start_ms as i64, end_ms as i64],
            feedback_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn feedback_for_sessions(
        &self,
        ids: &[String],
    ) -> Result<std::collections::HashMap<String, crate::feedback::types::FeedbackRecord>> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, session_id, score, label, note, created_at_ms
             FROM session_feedback WHERE session_id IN ({placeholders})
             ORDER BY created_at_ms DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), feedback_row)?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let r = row?;
            map.entry(r.session_id.clone()).or_insert(r);
        }
        Ok(map)
    }
}
