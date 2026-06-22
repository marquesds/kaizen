// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sync SQLite store. WAL mode, schema migrations as ordered SQL strings.

use crate::core::config::try_team_salt;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use crate::core::trace_span::{TraceSpanKind, TraceSpanRecord};
use crate::metrics::types::{
    FileFact, RankedFile, RankedTool, RepoEdge, RepoSnapshotRecord, ToolSpanView,
};
use crate::store::event_index::index_event_derived;
use crate::store::projector::{DEFAULT_ORPHAN_TTL_MS, Projector, ProjectorEvent};
use crate::store::tool_span_index::{
    clear_session_spans, rebuild_tool_spans_for_session, upsert_tool_span_record,
};
use crate::sync::context::SyncIngestContext;
use crate::sync::outbound::outbound_event_from_row;
use crate::sync::redact::redact_payload;
use crate::sync::smart::enqueue_tool_spans_for_session;
use anyhow::{Context, Result};
use rusqlite::types::Value;
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, TransactionBehavior, params, params_from_iter,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(super) use constants::{DEFAULT_CACHE_KIB, DEFAULT_MMAP_MB, SYNTHETIC_TS_CEILING_MS};
pub use constants::{
    SYNC_STATE_LAST_AGENT_SCAN_MS, SYNC_STATE_LAST_AUTO_PRUNE_MS, SYNC_STATE_SEARCH_DIRTY_MS,
};
pub(crate) use contracts::{CaptureQualityRow, TraceSpanQualityRow};
pub use contracts::{
    GuidanceKind, GuidancePerfRow, GuidanceReport, InsightsStats, PruneStats, SessionFilter,
    SessionOutcomeRow, SessionPage, SessionSampleAgg, SessionStatusRow, StoreOpenMode,
    SummaryStats, SyncStatusSnapshot, ToolSpanSyncRow,
};
pub(super) use sql::{PAIN_HOTSPOTS_SQL, SESSION_SELECT, TOOL_RANK_ROWS_SQL};
pub(crate) use visualization::SessionSearchQuery;

#[derive(Clone)]
struct SpanTreeCacheEntry {
    session_id: String,
    last_event_seq: Option<u64>,
    nodes: Vec<crate::store::span_tree::SpanNode>,
}

pub struct Store {
    conn: Connection,
    root: PathBuf,
    search_writer: RefCell<Option<crate::search::PendingWriter>>,
    span_tree_cache: RefCell<Option<SpanTreeCacheEntry>>,
    projector: RefCell<Projector>,
}

mod artifact_windows;
mod connection_functions;
mod constants;
mod contracts;
mod evals;
mod event_batch;
mod event_extensions;
mod event_projector;
mod event_read;
mod event_write;
mod events;
mod experiment_windows;
mod feedback;
mod guidance;
mod guidance_candidates;
mod maintenance;
mod metrics;
mod outbox_migration;
mod outcomes;
mod prompts;
mod report_windows;
mod reports;
mod rows;
mod samples;
mod schema;
mod session_identity;
mod session_read;
mod session_search_projection;
mod session_window;
mod sessions;
mod sql;
mod sync;
#[cfg(test)]
mod tests;
mod tool_span_sync;
mod tool_spans;
mod trace_spans;
mod visualization;

pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Store {
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_mode(path, StoreOpenMode::ReadWrite)
    }

    pub fn open_read_only(path: &Path) -> Result<Self> {
        Self::open_with_mode(path, StoreOpenMode::ReadOnlyQuery)
    }

    pub fn open_query(path: &Path) -> Result<Self> {
        Self::open_with_mode(path, StoreOpenMode::ReadOnlyQuery)
    }

    pub(crate) fn open_empty(root: &Path) -> Result<Self> {
        let conn = Connection::open_in_memory().context("open empty in-memory store")?;
        initialize_empty(&conn)?;
        Ok(store_from_connection(conn, root.to_path_buf()))
    }

    pub fn open_with_mode(path: &Path, mode: StoreOpenMode) -> Result<Self> {
        prepare_parent(path, mode)?;
        let conn = match mode {
            StoreOpenMode::ReadWrite => Connection::open(path),
            StoreOpenMode::ReadOnlyQuery => Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            ),
        }
        .with_context(|| format!("open db: {}", path.display()))?;
        schema::apply_pragmas(&conn, mode)?;
        connection_functions::register(&conn)?;
        if mode == StoreOpenMode::ReadWrite {
            for sql in schema::MIGRATIONS {
                conn.execute_batch(sql)?;
            }
            schema::ensure_schema_columns(&conn)?;
            session_identity::backfill(&conn)?;
            session_search_projection::backfill(&conn)?;
            outbox_migration::migrate(&conn, path.parent().unwrap_or_else(|| Path::new(".")))?;
        }
        let root = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Ok(store_from_connection(conn, root))
    }

    pub(super) fn invalidate_span_tree_cache(&self) {
        *self.span_tree_cache.borrow_mut() = None;
    }
}

fn initialize_empty(conn: &Connection) -> Result<()> {
    schema::apply_pragmas(conn, StoreOpenMode::ReadWrite)?;
    connection_functions::register(conn)?;
    schema::MIGRATIONS
        .iter()
        .try_for_each(|statement| conn.execute_batch(statement))?;
    schema::ensure_schema_columns(conn)
}

fn store_from_connection(conn: Connection, root: PathBuf) -> Store {
    Store {
        conn,
        root,
        search_writer: RefCell::new(None),
        span_tree_cache: RefCell::new(None),
        projector: RefCell::new(Projector::default()),
    }
}

fn prepare_parent(path: &Path, mode: StoreOpenMode) -> Result<()> {
    if mode == StoreOpenMode::ReadOnlyQuery {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

impl Drop for Store {
    fn drop(&mut self) {
        if let Some(writer) = self.search_writer.get_mut().as_mut() {
            let _ = writer.commit();
        }
    }
}
