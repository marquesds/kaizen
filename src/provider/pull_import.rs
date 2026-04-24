// SPDX-License-Identifier: AGPL-3.0-or-later
//! Map provider [`PullPage`] items into `remote_events` when JSON matches [`OutboundEvent`].

use super::PullPage;
use crate::store::Store;
use crate::sync::outbound::OutboundEvent;
use anyhow::Result;

/// Parse each `page.items` entry as an [`OutboundEvent`]; on success, upsert into `remote_events`.
/// Malformed or non-event JSON rows are skipped. Returns the number of rows written.
pub fn import_pull_page_to_remote(
    store: &Store,
    team_id: &str,
    workspace_hash: &str,
    page: &PullPage,
) -> Result<usize> {
    if team_id.trim().is_empty() || workspace_hash.trim().is_empty() {
        return Ok(0);
    }
    let mut n = 0;
    for v in &page.items {
        let o: OutboundEvent = match serde_json::from_value(v.clone()) {
            Ok(x) => x,
            Err(_) => continue,
        };
        let json = serde_json::to_string(&o)?;
        store.remote_insert_event(
            team_id,
            workspace_hash,
            &o.session_id_hash,
            o.event_seq as i64,
            &json,
        )?;
        n += 1;
    }
    Ok(n)
}
