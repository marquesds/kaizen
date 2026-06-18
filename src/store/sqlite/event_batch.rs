use super::*;

impl Store {
    pub(crate) fn append_scanned_event_batch(
        &self,
        events: &[Event],
        ctx: Option<&SyncIngestContext>,
        flush_ms: Option<u64>,
    ) -> Result<usize> {
        let Some(session_id) = batch_session(events)? else {
            return Ok(0);
        };
        let transaction = self.conn.unchecked_transaction()?;
        events
            .iter()
            .try_for_each(|event| self.append_event_deferred(event, ctx))?;
        self.finish_scanned_batch(session_id, flush_ms)?;
        transaction.commit()?;
        Ok(events.len())
    }

    fn finish_scanned_batch(&self, session_id: &str, flush_ms: Option<u64>) -> Result<()> {
        if let Some(timestamp) = flush_ms {
            self.flush_projector_session(session_id, timestamp)?;
        }
        self.refresh_extension_session(session_id)
    }
}

fn batch_session(events: &[Event]) -> Result<Option<&str>> {
    let Some(first) = events.first() else {
        return Ok(None);
    };
    anyhow::ensure!(
        events
            .iter()
            .all(|event| event.session_id == first.session_id),
        "scanned event batch mixes sessions"
    );
    Ok(Some(&first.session_id))
}
