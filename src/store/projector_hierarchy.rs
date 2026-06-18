// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure hierarchy decisions for incremental projection.

use crate::store::tool_span_index::SpanBuilder;
use std::collections::HashMap;

pub(crate) fn open_parent(
    order: &[String],
    spans: &HashMap<String, SpanBuilder>,
    span_id: &str,
) -> (Option<String>, u32) {
    order
        .iter()
        .rev()
        .filter(|id| id.as_str() != span_id)
        .find_map(|id| spans.get(id).map(|span| (Some(id.clone()), span.depth + 1)))
        .unwrap_or((None, 0))
}

pub(crate) fn seed_subtree(span: &mut SpanBuilder) {
    span.subtree_cost_usd_e6 = add_i64(span.subtree_cost_usd_e6, span.cost_usd_e6);
    let own_tokens = span
        .tokens_in
        .map(|value| value + span.tokens_out.unwrap_or(0));
    span.subtree_token_count = add_u32(span.subtree_token_count, own_tokens);
}

pub(crate) fn add_child(parent: &mut SpanBuilder, child: &SpanBuilder) {
    parent.subtree_cost_usd_e6 = add_i64(parent.subtree_cost_usd_e6, child.subtree_cost_usd_e6);
    parent.subtree_token_count = add_u32(parent.subtree_token_count, child.subtree_token_count);
}

pub(crate) fn detach_chain(
    order: &mut Vec<String>,
    spans: &mut HashMap<String, SpanBuilder>,
    span: &SpanBuilder,
) {
    let Some(index) = order.iter().position(|id| id == &span.span_id) else {
        return;
    };
    order.remove(index);
    if let Some(child) = order.get(index).and_then(|id| spans.get_mut(id)) {
        child.parent_span_id = span.parent_span_id.clone();
    }
    order.iter().skip(index).for_each(|id| {
        if let Some(item) = spans.get_mut(id) {
            item.depth = item.depth.saturating_sub(1);
        }
    });
}

fn add_i64(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    right.map(|value| left.unwrap_or(0) + value).or(left)
}

fn add_u32(left: Option<u32>, right: Option<u32>) -> Option<u32> {
    right.map(|value| left.unwrap_or(0) + value).or(left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtree_adds_child_totals_once() {
        let mut parent = SpanBuilder::default();
        let child = SpanBuilder {
            subtree_cost_usd_e6: Some(7),
            subtree_token_count: Some(11),
            ..Default::default()
        };
        add_child(&mut parent, &child);
        assert_eq!(parent.subtree_cost_usd_e6, Some(7));
        assert_eq!(parent.subtree_token_count, Some(11));
    }
}
