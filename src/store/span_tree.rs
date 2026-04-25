// SPDX-License-Identifier: AGPL-3.0-or-later
//! In-memory span tree assembled from flat `ToolSpanView` rows.

use crate::metrics::types::ToolSpanView;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanNode {
    pub span: ToolSpanView,
    pub children: Vec<SpanNode>,
    pub subtree_cost_usd_e6: i64,
    pub subtree_token_count: u64,
}

/// Assemble a forest of `SpanNode` from a flat ordered list.
pub fn build_tree(spans: Vec<ToolSpanView>) -> Vec<SpanNode> {
    let ids: Vec<String> = spans.iter().map(|s| s.span_id.clone()).collect();
    let mut nodes: HashMap<String, SpanNode> = spans
        .into_iter()
        .map(|s| {
            let cost = s.subtree_cost_usd_e6.unwrap_or(0);
            let tokens = s.subtree_token_count.unwrap_or(0) as u64;
            (s.span_id.clone(), SpanNode { span: s, children: vec![], subtree_cost_usd_e6: cost, subtree_token_count: tokens })
        })
        .collect();
    let mut roots: Vec<String> = Vec::new();
    for id in &ids {
        let pid = nodes[id].span.parent_span_id.clone();
        match pid {
            Some(p) if nodes.contains_key(&p) => {
                let child = nodes.remove(id).expect("id present");
                nodes.get_mut(&p).expect("parent present").children.push(child);
            }
            _ => roots.push(id.clone()),
        }
    }
    roots.into_iter().filter_map(|id| nodes.remove(&id)).collect()
}
