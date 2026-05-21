use super::*;

pub(super) fn old_session_ids(
    tx: &rusqlite::Transaction<'_>,
    cutoff_ms: i64,
) -> Result<Vec<String>> {
    let mut stmt = tx.prepare("SELECT id FROM sessions WHERE started_at_ms < ?1")?;
    let rows = stmt.query_map(params![cutoff_ms], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

impl Store {
    /// Delete sessions with `started_at_ms` strictly before `cutoff_ms` and all dependent rows.
    pub fn prune_sessions_started_before(&self, cutoff_ms: i64) -> Result<PruneStats> {
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred)?;
        let old_ids = old_session_ids(&tx, cutoff_ms)?;
        let sessions_to_remove: i64 = tx.query_row(
            "SELECT COUNT(*) FROM sessions WHERE started_at_ms < ?1",
            params![cutoff_ms],
            |r| r.get(0),
        )?;
        let events_to_remove: i64 = tx.query_row(
            "SELECT COUNT(*) FROM events WHERE session_id IN \
             (SELECT id FROM sessions WHERE started_at_ms < ?1)",
            params![cutoff_ms],
            |r| r.get(0),
        )?;

        let sub_old_sessions = "SELECT id FROM sessions WHERE started_at_ms < ?1";
        tx.execute(
            &format!(
                "DELETE FROM tool_span_paths WHERE span_id IN \
                 (SELECT span_id FROM tool_spans WHERE session_id IN ({sub_old_sessions}))"
            ),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM tool_spans WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM events WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM files_touched WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM skills_used WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM rules_used WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM sync_outbox WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM session_repo_binding WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM experiment_tags WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM session_outcomes WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            &format!("DELETE FROM session_samples WHERE session_id IN ({sub_old_sessions})"),
            params![cutoff_ms],
        )?;
        tx.execute(
            "DELETE FROM sessions WHERE started_at_ms < ?1",
            params![cutoff_ms],
        )?;
        tx.commit()?;
        if let Some(mut writer) = self.search_writer.borrow_mut().take() {
            let _ = writer.commit();
        }
        if let Err(err) = crate::search::delete_sessions(&self.root, &old_ids) {
            tracing::warn!("search prune skipped: {err:#}");
            let _ = self.sync_state_set_u64(SYNC_STATE_SEARCH_DIRTY_MS, now_ms());
        }
        self.invalidate_span_tree_cache();
        Ok(PruneStats {
            sessions_removed: sessions_to_remove as u64,
            events_removed: events_to_remove as u64,
        })
    }

    /// Reclaim file space after large deletes (exclusive lock; can be slow).
    pub fn vacuum(&self) -> Result<()> {
        self.conn.execute_batch("VACUUM;").context("VACUUM")?;
        Ok(())
    }
}
