// SPDX-License-Identifier: AGPL-3.0-or-later
//! Single daemon worker. All request execution funnels through this thread.

use crate::ipc::{DaemonRequest, DaemonResponse, SessionDetail};
use crate::store::Store;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

const MAX_CACHED_STORES: usize = 8;

pub(super) struct Job {
    pub(super) request: DaemonRequest,
    pub(super) reply: oneshot::Sender<DaemonResponse>,
}

pub(super) fn spawn_worker(
    mut rx: mpsc::Receiver<Job>,
    queue_depth: Arc<AtomicUsize>,
    last_error: Arc<Mutex<Option<String>>>,
) {
    std::thread::spawn(move || {
        let mut stores = StoreCache::default();
        while let Some(job) = rx.blocking_recv() {
            let response = worker_response(job.request, &last_error, &mut stores);
            queue_depth.fetch_sub(1, Ordering::Relaxed);
            let _ = job.reply.send(response);
        }
    });
}

fn worker_response(
    request: DaemonRequest,
    last_error: &Arc<Mutex<Option<String>>>,
    stores: &mut StoreCache,
) -> DaemonResponse {
    match handle_request(request, stores) {
        Ok(response) => response,
        Err(err) => {
            let message = format!("{err:#}");
            if let Ok(mut slot) = last_error.lock() {
                *slot = Some(message.clone());
            }
            DaemonResponse::Error {
                message,
                supported_min: None,
                supported_max: None,
            }
        }
    }
}

fn handle_request(request: DaemonRequest, stores: &mut StoreCache) -> Result<DaemonResponse> {
    match request {
        DaemonRequest::Stop => Ok(DaemonResponse::Ack {
            message: "stopping".to_string(),
        }),
        DaemonRequest::ListSessions {
            workspace,
            offset,
            limit,
            filter,
        } => {
            let page = stores
                .workspace_read(&PathBuf::from(&workspace))?
                .map(|store| store.list_sessions_page(&workspace, offset, limit, filter))
                .transpose()?
                .unwrap_or_else(empty_page);
            Ok(DaemonResponse::Sessions(page))
        }
        DaemonRequest::GetSessionDetail { id, workspace } => load_detail(id, workspace, stores),
        DaemonRequest::IngestHook {
            source,
            payload,
            workspace,
        } => {
            let ws = workspace_path(workspace)?;
            let store = stores.workspace(&ws)?;
            crate::shell::ingest::ingest_hook_with_store(source, &payload, &ws, store)?;
            store.flush_search().ok();
            Ok(DaemonResponse::Ack {
                message: "ingested".to_string(),
            })
        }
        DaemonRequest::Hello(_)
        | DaemonRequest::Status
        | DaemonRequest::EnsureWorkspaceCapture { .. }
        | DaemonRequest::EnsureProxy { .. }
        | DaemonRequest::BeginObservedSession { .. } => unreachable!(),
    }
}

fn load_detail(
    id: String,
    workspace: Option<String>,
    stores: &mut StoreCache,
) -> Result<DaemonResponse> {
    let roots = workspace
        .map(PathBuf::from)
        .map(|p| vec![p])
        .unwrap_or_else(|| crate::core::machine_registry::list_paths().unwrap_or_default());
    for ws in roots {
        let Some(store) = stores.workspace_read(&ws)? else {
            continue;
        };
        if let Some(session) = store.get_session(&id)? {
            return Ok(DaemonResponse::Detail(Box::new(SessionDetail {
                session: Some(session),
                events: store.list_events_for_session(&id)?,
                spans: store.session_span_tree(&id)?,
            })));
        }
    }
    Ok(DaemonResponse::Detail(Box::new(SessionDetail {
        session: None,
        events: Vec::new(),
        spans: Vec::new(),
    })))
}

fn empty_page() -> crate::store::SessionPage {
    crate::store::SessionPage {
        rows: Vec::new(),
        total: 0,
        next_offset: None,
    }
}

fn workspace_path(workspace: Option<String>) -> Result<PathBuf> {
    let path = workspace
        .map(PathBuf::from)
        .unwrap_or(std::env::current_dir()?);
    Ok(crate::core::paths::canonical(&path))
}

#[derive(Default)]
struct StoreCache {
    stores: HashMap<PathBuf, CachedStore>,
}

struct CachedStore {
    store: Store,
    writable: bool,
}

impl StoreCache {
    fn workspace(&mut self, workspace: &Path) -> Result<&Store> {
        let path = crate::core::workspace::db_path(workspace)?;
        self.open_write(&path)
    }

    fn workspace_read(&mut self, workspace: &Path) -> Result<Option<&Store>> {
        let path = crate::core::workspace::db_path(workspace)?;
        if !path.exists() {
            return Ok(None);
        }
        self.open_read(&path).map(Some)
    }

    fn open_write(&mut self, path: &Path) -> Result<&Store> {
        if !self.stores.get(path).is_some_and(|entry| entry.writable) {
            self.insert(path.to_path_buf(), true)?;
        }
        Ok(&self.stores.get(path).expect("cached store").store)
    }

    fn open_read(&mut self, path: &Path) -> Result<&Store> {
        if !self.stores.contains_key(path) {
            self.insert(path.to_path_buf(), false)?;
        }
        Ok(&self.stores.get(path).expect("cached store").store)
    }

    fn insert(&mut self, path: PathBuf, writable: bool) -> Result<()> {
        if !self.stores.contains_key(&path) && self.stores.len() >= MAX_CACHED_STORES {
            self.evict_one();
        }
        let store = if writable {
            Store::open(&path)?
        } else {
            Store::open_query(&path)?
        };
        self.stores.insert(path, CachedStore { store, writable });
        Ok(())
    }

    fn evict_one(&mut self) {
        if let Some(path) = self.stores.keys().next().cloned() {
            self.stores.remove(&path);
        }
    }
}

#[cfg(test)]
mod tests;
