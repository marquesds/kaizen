// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cheap per-connection SQLite revision tracking for live Web updates.

use anyhow::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Default)]
pub(super) struct Subscription {
    workspace: Option<String>,
    revision: Option<Revision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Revision {
    database: FileStamp,
    wal: FileStamp,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FileStamp {
    len: u64,
    modified_ns: u128,
}

impl Subscription {
    pub(super) fn set(&mut self, workspace: Option<String>) -> Result<()> {
        self.revision = workspace.as_deref().map(revision).transpose()?;
        self.workspace = workspace;
        Ok(())
    }

    pub(super) fn clear(&mut self) {
        self.workspace = None;
        self.revision = None;
    }

    pub(super) fn is_active(&self) -> bool {
        self.workspace.is_some()
    }

    pub(super) fn changed(&mut self) -> Option<Value> {
        let workspace = self.workspace.as_deref()?;
        let next = revision(workspace).ok()?;
        if self.revision == Some(next) {
            return None;
        }
        self.revision = Some(next);
        Some(json!({"type":"changed", "workspace":workspace}))
    }
}

fn revision(workspace: &str) -> Result<Revision> {
    let database = crate::core::workspace::db_path(Path::new(workspace))?;
    let wal = sidecar(&database, "-wal");
    Ok(Revision {
        database: stamp(&database),
        wal: stamp(&wal),
    })
}

fn sidecar(database: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{suffix}", database.to_string_lossy()))
}

fn stamp(path: &Path) -> FileStamp {
    path.metadata().map_or_else(
        |_| FileStamp::default(),
        |meta| FileStamp {
            len: meta.len(),
            modified_ns: meta.modified().ok().and_then(since_epoch).unwrap_or(0),
        },
    )
}

fn since_epoch(time: std::time::SystemTime) -> Option<u128> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_nanos())
}
