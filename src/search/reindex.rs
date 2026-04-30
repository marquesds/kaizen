// SPDX-License-Identifier: AGPL-3.0-or-later
//! Drop and rebuild search index from persisted events.

use crate::core::config::try_team_salt;
use crate::core::event::{Event, SessionRecord};
use crate::search::extract_doc;
use crate::search::writer::{PendingWriter, index_dir};
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ReindexStats {
    pub events_seen: u64,
    pub docs_indexed: u64,
}

pub fn reindex_workspace(
    root: &Path,
    workspace: &Path,
    sessions: &[SessionRecord],
    events: Vec<(SessionRecord, Event)>,
    cfg: &crate::core::config::Config,
) -> Result<ReindexStats> {
    drop_index(root)?;
    let salt = try_team_salt(&cfg.sync).unwrap_or([0; 32]);
    let session_map = sessions
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut writer = PendingWriter::open(root)?;
    let mut stats = ReindexStats {
        events_seen: events.len() as u64,
        docs_indexed: 0,
    };
    for (_, event) in events {
        let Some(session) = session_map.get(&event.session_id) else {
            continue;
        };
        if let Some(doc) = extract_doc(&event, session, workspace, &salt) {
            writer.add(&doc)?;
            stats.docs_indexed += 1;
        }
    }
    writer.commit()?;
    Ok(stats)
}

fn drop_index(root: &Path) -> Result<()> {
    let dir = index_dir(root);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
