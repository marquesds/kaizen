// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::store::span_tree::SpanNode;
use std::collections::HashMap;

#[derive(Clone)]
pub struct DetailData {
    pub tool_lead_by_call: HashMap<String, u64>,
    pub span_nodes: Vec<SpanNode>,
}

pub enum DetailState {
    Idle,
    Loading { token: u64, session_id: String },
    Ready(DetailData),
    Error(String),
}

impl DetailState {
    pub fn leads(&self) -> &HashMap<String, u64> {
        match self {
            Self::Ready(data) => &data.tool_lead_by_call,
            _ => empty_leads(),
        }
    }

    pub fn spans(&self) -> &[SpanNode] {
        match self {
            Self::Ready(data) => &data.span_nodes,
            _ => &[],
        }
    }

    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error(err) => Some(err),
            _ => None,
        }
    }
}

fn empty_leads() -> &'static HashMap<String, u64> {
    static EMPTY: std::sync::OnceLock<HashMap<String, u64>> = std::sync::OnceLock::new();
    EMPTY.get_or_init(HashMap::new)
}
