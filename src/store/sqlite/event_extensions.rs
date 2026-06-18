use super::*;

impl Store {
    pub(super) fn index_extension_event(&self, event: &Event) -> Result<()> {
        crate::extensions::hash_chain::store_event_hash(self, event)
    }

    pub(super) fn apply_live_extension_event(&self, event: &Event) -> Result<()> {
        crate::extensions::aggregates::apply_event(self, event)
    }

    pub(super) fn refresh_extension_session(&self, session_id: &str) -> Result<()> {
        crate::extensions::aggregates::upsert_session(self, session_id)?;
        if let Err(error) = crate::extensions::diffs::refresh_session(self, session_id, false) {
            tracing::warn!(%session_id, "step diff attribution skipped: {error:#}");
        }
        Ok(())
    }
}
