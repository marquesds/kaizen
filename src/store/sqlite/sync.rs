use super::*;

impl Store {
    pub(super) fn append_outbox_row(
        &self,
        owner_id: &str,
        kind: &str,
        payload: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_outbox (session_id, kind, payload, sent)
             VALUES (?1, ?2, ?3, 0)",
            params![owner_id, kind, payload],
        )?;
        Ok(())
    }

    pub fn list_outbox_pending(&self, limit: usize) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, payload FROM sync_outbox WHERE sent = 0 ORDER BY id ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], read_outbox_row)?;
        rows.collect::<rusqlite::Result<_>>().map_err(Into::into)
    }

    pub fn mark_outbox_sent(&self, ids: &[i64]) -> Result<()> {
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        ids.iter().try_for_each(|id| {
            tx.execute("UPDATE sync_outbox SET sent = 1 WHERE id = ?1", [id])
                .map(|_| ())
        })?;
        tx.commit()?;
        Ok(())
    }

    pub fn replace_outbox_rows(
        &self,
        owner_id: &str,
        kind: &str,
        payloads: &[String],
    ) -> Result<()> {
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        tx.execute(
            "DELETE FROM sync_outbox WHERE session_id = ?1 AND kind = ?2 AND sent = 0",
            params![owner_id, kind],
        )?;
        insert_outbox_payloads(&tx, owner_id, kind, payloads)?;
        tx.commit()?;
        Ok(())
    }

    pub fn outbox_pending_count(&self) -> Result<u64> {
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

fn read_outbox_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, String, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn insert_outbox_payloads(
    tx: &rusqlite::Transaction<'_>,
    owner_id: &str,
    kind: &str,
    payloads: &[String],
) -> rusqlite::Result<()> {
    payloads.iter().try_for_each(|payload| {
        tx.execute(
            "INSERT INTO sync_outbox (session_id, kind, payload, sent)
             VALUES (?1, ?2, ?3, 0)",
            params![owner_id, kind, payload],
        )
        .map(|_| ())
    })
}
