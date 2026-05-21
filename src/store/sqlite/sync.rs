use super::*;

impl Store {
    pub fn list_outbox_pending(&self, limit: usize) -> Result<Vec<(i64, String, String)>> {
        let rows = self.outbox()?.list_pending(limit)?;
        if !rows.is_empty() {
            return Ok(rows);
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, payload FROM sync_outbox WHERE sent = 0 ORDER BY id ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn mark_outbox_sent(&self, ids: &[i64]) -> Result<()> {
        self.outbox()?.delete_ids(ids)?;
        for id in ids {
            self.conn
                .execute("UPDATE sync_outbox SET sent = 1 WHERE id = ?1", params![id])?;
        }
        Ok(())
    }

    pub fn replace_outbox_rows(
        &self,
        owner_id: &str,
        kind: &str,
        payloads: &[String],
    ) -> Result<()> {
        self.outbox()?.replace(owner_id, kind, payloads)?;
        self.conn.execute(
            "DELETE FROM sync_outbox WHERE session_id = ?1 AND kind = ?2 AND sent = 0",
            params![owner_id, kind],
        )?;
        for payload in payloads {
            self.conn.execute(
                "INSERT INTO sync_outbox (session_id, kind, payload, sent) VALUES (?1, ?2, ?3, 0)",
                params![owner_id, kind, payload],
            )?;
        }
        Ok(())
    }

    pub fn outbox_pending_count(&self) -> Result<u64> {
        let redb = self.outbox()?.pending_count()?;
        if redb > 0 {
            return Ok(redb);
        }
        let c: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM sync_outbox WHERE sent = 0", [], |r| {
                    r.get(0)
                })?;
        Ok(c as u64)
    }

    pub fn set_sync_state_ok(&self) -> Result<()> {
        let now = now_ms().to_string();
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('last_success_ms', ?1)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![now],
        )?;
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('consecutive_failures', '0')
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            [],
        )?;
        self.conn
            .execute("DELETE FROM sync_state WHERE k = 'last_error'", [])?;
        Ok(())
    }

    pub fn set_sync_state_error(&self, msg: &str) -> Result<()> {
        let prev: i64 = self
            .conn
            .query_row(
                "SELECT v FROM sync_state WHERE k = 'consecutive_failures'",
                [],
                |r| {
                    let s: String = r.get(0)?;
                    Ok(s.parse::<i64>().unwrap_or(0))
                },
            )
            .optional()?
            .unwrap_or(0);
        let next = prev.saturating_add(1);
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('last_error', ?1)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![msg],
        )?;
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES ('consecutive_failures', ?1)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![next.to_string()],
        )?;
        Ok(())
    }

    pub fn sync_status(&self) -> Result<SyncStatusSnapshot> {
        let pending_outbox = self.outbox_pending_count()?;
        let last_success_ms = self
            .conn
            .query_row(
                "SELECT v FROM sync_state WHERE k = 'last_success_ms'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .and_then(|s| s.parse().ok());
        let last_error = self
            .conn
            .query_row("SELECT v FROM sync_state WHERE k = 'last_error'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        let consecutive_failures = self
            .conn
            .query_row(
                "SELECT v FROM sync_state WHERE k = 'consecutive_failures'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        Ok(SyncStatusSnapshot {
            pending_outbox,
            last_success_ms,
            last_error,
            consecutive_failures,
        })
    }

    pub fn sync_state_get_u64(&self, key: &str) -> Result<Option<u64>> {
        let row: Option<String> = self
            .conn
            .query_row("SELECT v FROM sync_state WHERE k = ?1", params![key], |r| {
                r.get::<_, String>(0)
            })
            .optional()?;
        Ok(row.and_then(|s| s.parse().ok()))
    }

    pub fn sync_state_set_u64(&self, key: &str, v: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_state (k, v) VALUES (?1, ?2)
             ON CONFLICT(k) DO UPDATE SET v = excluded.v",
            params![key, v.to_string()],
        )?;
        Ok(())
    }
}
