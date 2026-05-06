// SPDX-License-Identifier: AGPL-3.0-or-later
//! Single daemon worker. All request execution funnels through this thread.

use crate::ipc::{DaemonRequest, DaemonResponse, SessionDetail};
use crate::store::Store;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

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
        while let Some(job) = rx.blocking_recv() {
            let response = worker_response(job.request, &last_error);
            queue_depth.fetch_sub(1, Ordering::Relaxed);
            let _ = job.reply.send(response);
        }
    });
}

fn worker_response(
    request: DaemonRequest,
    last_error: &Arc<Mutex<Option<String>>>,
) -> DaemonResponse {
    match handle_request(request) {
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

fn handle_request(request: DaemonRequest) -> Result<DaemonResponse> {
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
            let store = Store::open(&crate::core::workspace::db_path(&PathBuf::from(
                &workspace,
            ))?)?;
            let page = store.list_sessions_page(&workspace, offset, limit, filter)?;
            Ok(DaemonResponse::Sessions(page))
        }
        DaemonRequest::GetSessionDetail { id, workspace } => load_detail(id, workspace),
        DaemonRequest::IngestHook {
            source,
            payload,
            workspace,
        } => {
            let workspace = workspace.map(|p| crate::core::paths::canonical(&PathBuf::from(p)));
            crate::shell::ingest::ingest_hook_text(source, &payload, workspace.clone())?;
            if let Some(ws) = workspace {
                let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
                store.flush_search().ok();
            }
            Ok(DaemonResponse::Ack {
                message: "ingested".to_string(),
            })
        }
        DaemonRequest::Hello(_) | DaemonRequest::Status => unreachable!(),
    }
}

fn load_detail(id: String, workspace: Option<String>) -> Result<DaemonResponse> {
    let roots = workspace
        .map(PathBuf::from)
        .map(|p| vec![p])
        .unwrap_or_else(|| crate::core::machine_registry::list_paths().unwrap_or_default());
    for ws in roots {
        let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
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
