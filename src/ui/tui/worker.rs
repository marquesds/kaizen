// SPDX-License-Identifier: AGPL-3.0-or-later
//! Blocking SQLite worker for TUI. UI thread never waits on large queries.

use super::view::DetailData;
use crate::core::event::Event;
use crate::store::{SessionFilter, SessionPage, Store};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub enum StoreRequest {
    SessionsPage {
        token: u64,
        workspace: String,
        offset: usize,
        limit: usize,
        filter: SessionFilter,
    },
    EventsPage {
        token: u64,
        session_id: String,
        after_seq: u64,
        limit: usize,
    },
    Detail {
        token: u64,
        session_id: String,
    },
    Feedback {
        token: u64,
        ids: Vec<String>,
    },
}

pub enum StoreResponse {
    SessionsPage {
        token: u64,
        offset: usize,
        result: Result<SessionPage, String>,
    },
    EventsPage {
        token: u64,
        session_id: String,
        after_seq: u64,
        result: Result<Vec<Event>, String>,
    },
    Detail {
        token: u64,
        session_id: String,
        result: Result<DetailData, String>,
    },
    Feedback {
        token: u64,
        result: Result<HashMap<String, u8>, String>,
    },
}

pub fn spawn_store_worker(
    db_path: PathBuf,
) -> (
    mpsc::UnboundedSender<StoreRequest>,
    mpsc::UnboundedReceiver<StoreResponse>,
) {
    let (req_tx, mut req_rx) = mpsc::unbounded_channel();
    let (res_tx, res_rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        let store = match Store::open_read_only(&db_path) {
            Ok(store) => store,
            Err(err) => {
                while let Some(req) = req_rx.blocking_recv() {
                    let _ = res_tx.send(open_error_response(req, &err.to_string()));
                }
                return;
            }
        };
        while let Some(req) = req_rx.blocking_recv() {
            let response = handle_request(&store, req);
            let _ = res_tx.send(response);
        }
    });
    (req_tx, res_rx)
}

fn open_error_response(req: StoreRequest, error: &str) -> StoreResponse {
    match req {
        StoreRequest::SessionsPage { token, offset, .. } => StoreResponse::SessionsPage {
            token,
            offset,
            result: Err(error.to_string()),
        },
        StoreRequest::EventsPage {
            token,
            session_id,
            after_seq,
            ..
        } => StoreResponse::EventsPage {
            token,
            session_id,
            after_seq,
            result: Err(error.to_string()),
        },
        StoreRequest::Detail {
            token, session_id, ..
        } => StoreResponse::Detail {
            token,
            session_id,
            result: Err(error.to_string()),
        },
        StoreRequest::Feedback { token, .. } => StoreResponse::Feedback {
            token,
            result: Err(error.to_string()),
        },
    }
}

fn handle_request(store: &Store, req: StoreRequest) -> StoreResponse {
    match req {
        StoreRequest::SessionsPage {
            token,
            workspace,
            offset,
            limit,
            filter,
        } => {
            let result = store
                .list_sessions_page(&workspace, offset, limit, filter)
                .map_err(|err| err.to_string());
            StoreResponse::SessionsPage {
                token,
                offset,
                result,
            }
        }
        StoreRequest::EventsPage {
            token,
            session_id,
            after_seq,
            limit,
        } => {
            let result = store
                .list_events_page(&session_id, after_seq, limit)
                .map_err(|err| err.to_string());
            StoreResponse::EventsPage {
                token,
                session_id,
                after_seq,
                result,
            }
        }
        StoreRequest::Detail { token, session_id } => {
            let result = load_detail(store, &session_id).map_err(|err| err.to_string());
            StoreResponse::Detail {
                token,
                session_id,
                result,
            }
        }
        StoreRequest::Feedback { token, ids } => {
            let result = load_feedback(store, &ids).map_err(|err| err.to_string());
            StoreResponse::Feedback { token, result }
        }
    }
}

fn load_detail(store: &Store, session_id: &str) -> anyhow::Result<DetailData> {
    let mut tool_lead_by_call = HashMap::new();
    for row in store.tool_spans_for_session(session_id)? {
        if let (Some(id), Some(lead)) = (row.tool_call_id, row.lead_time_ms) {
            tool_lead_by_call.insert(id, lead);
        }
    }
    Ok(DetailData {
        tool_lead_by_call,
        span_nodes: store.session_span_tree(session_id).unwrap_or_default(),
    })
}

fn load_feedback(store: &Store, ids: &[String]) -> anyhow::Result<HashMap<String, u8>> {
    Ok(store
        .feedback_for_sessions(ids)?
        .into_iter()
        .filter_map(|(sid, row)| row.score.map(|score| (sid, score.0)))
        .collect())
}
