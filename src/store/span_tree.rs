// SPDX-License-Identifier: AGPL-3.0-or-later
//! In-memory span tree assembled from flat `ToolSpanView` rows.

use crate::metrics::types::ToolSpanView;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanNode {
    pub span: ToolSpanView,
    pub children: Vec<SpanNode>,
    pub subtree_cost_usd_e6: i64,
    pub subtree_token_count: u64,
}

/// Assemble a forest of `SpanNode` from a flat ordered list.
pub fn build_tree(spans: Vec<ToolSpanView>) -> Vec<SpanNode> {
    let input = SpanInput::new(spans);
    let mut seen = HashSet::new();
    let mut roots = build_ids(input.root_ids(), &input, &mut seen);
    roots.extend(build_ids(input.remaining_ids(&seen), &input, &mut seen));
    roots
}

struct SpanInput {
    order: Vec<String>,
    nodes: HashMap<String, ToolSpanView>,
    children: HashMap<String, Vec<String>>,
}

impl SpanInput {
    fn new(spans: Vec<ToolSpanView>) -> Self {
        let mut input = Self {
            order: vec![],
            nodes: HashMap::new(),
            children: HashMap::new(),
        };
        spans.into_iter().for_each(|span| input.insert(span));
        input.link_children();
        input
    }

    fn insert(&mut self, span: ToolSpanView) {
        if self.nodes.contains_key(&span.span_id) {
            return;
        }
        self.order.push(span.span_id.clone());
        self.nodes.insert(span.span_id.clone(), span);
    }

    fn link_children(&mut self) {
        let edges: Vec<_> = self
            .order
            .iter()
            .filter_map(|id| self.parent_edge(id))
            .collect();
        edges.into_iter().for_each(|(p, c)| {
            self.children.entry(p).or_default().push(c);
        });
    }

    fn parent_edge(&self, id: &str) -> Option<(String, String)> {
        let parent = self.nodes[id].parent_span_id.as_ref()?;
        self.nodes
            .contains_key(parent)
            .then(|| (parent.clone(), id.into()))
    }

    fn root_ids(&self) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| self.is_root(id))
            .cloned()
            .collect()
    }

    fn is_root(&self, id: &str) -> bool {
        self.nodes[id]
            .parent_span_id
            .as_ref()
            .is_none_or(|p| !self.nodes.contains_key(p))
    }

    fn remaining_ids(&self, seen: &HashSet<String>) -> Vec<String> {
        self.order
            .iter()
            .filter(|id| !seen.contains(*id))
            .cloned()
            .collect()
    }
}

fn build_ids(ids: Vec<String>, input: &SpanInput, seen: &mut HashSet<String>) -> Vec<SpanNode> {
    ids.into_iter()
        .filter_map(|id| assemble(&id, input, seen, &mut vec![]))
        .collect()
}

fn assemble(
    id: &str,
    input: &SpanInput,
    seen: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Option<SpanNode> {
    if seen.contains(id) || stack.iter().any(|item| item == id) {
        return None;
    }
    let span = input.nodes.get(id)?.clone();
    stack.push(id.into());
    let children = child_nodes(id, input, seen, stack);
    stack.pop();
    seen.insert(id.into());
    Some(span_node(span, children))
}

fn child_nodes(
    id: &str,
    input: &SpanInput,
    seen: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Vec<SpanNode> {
    input
        .children
        .get(id)
        .into_iter()
        .flatten()
        .filter_map(|child| assemble(child, input, seen, stack))
        .collect()
}

fn span_node(span: ToolSpanView, children: Vec<SpanNode>) -> SpanNode {
    let subtree_cost_usd_e6 = span.subtree_cost_usd_e6.unwrap_or_default();
    let subtree_token_count = span.subtree_token_count.unwrap_or_default() as u64;
    SpanNode {
        span,
        children,
        subtree_cost_usd_e6,
        subtree_token_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn build_tree_keeps_grandchildren() {
        let roots = build_tree(vec![
            span("root", None),
            span("child", Some("root")),
            span("grandchild", Some("child")),
        ]);
        assert_eq!(ids(&roots), vec!["root"]);
        assert_eq!(ids(&roots[0].children), vec!["child"]);
        assert_eq!(ids(&roots[0].children[0].children), vec!["grandchild"]);
    }

    #[test]
    fn build_tree_keeps_missing_parent_and_cycle_nodes_once() {
        let roots = build_tree(vec![
            span("orphan", Some("missing")),
            span("a", Some("b")),
            span("b", Some("a")),
        ]);
        assert_eq!(flat_ids(&roots), vec!["orphan", "a", "b"]);
    }

    fn span(id: &str, parent: Option<&str>) -> ToolSpanView {
        ToolSpanView {
            span_id: id.into(),
            parent_span_id: parent.map(str::to_string),
            ..Default::default()
        }
    }

    fn ids(nodes: &[SpanNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.span.span_id.as_str()).collect()
    }

    fn flat_ids(nodes: &[SpanNode]) -> Vec<&str> {
        nodes
            .iter()
            .flat_map(|n| std::iter::once(n.span.span_id.as_str()).chain(flat_ids(&n.children)))
            .collect()
    }
}
