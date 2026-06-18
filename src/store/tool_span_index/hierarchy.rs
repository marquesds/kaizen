// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{SpanBuilder, span_end, span_start};
use std::{cmp::Reverse, collections::HashMap};

#[derive(Clone, Copy)]
struct ParentCandidate {
    index: usize,
    depth: u32,
}

impl ParentCandidate {
    fn prefer(self, other: Self) -> Self {
        if (self.depth, Reverse(self.index)) >= (other.depth, Reverse(other.index)) {
            self
        } else {
            other
        }
    }
}

struct ParentIndex {
    ends: Vec<u64>,
    // Fenwick prefix over descending end ranks: candidates ending at/after child.
    tree: Vec<Option<ParentCandidate>>,
}

impl ParentIndex {
    fn new(spans: &[SpanBuilder]) -> Self {
        let ends = unique_span_ends(spans);
        Self {
            tree: vec![None; ends.len() + 1],
            ends,
        }
    }

    fn rank(&self, end: u64) -> usize {
        self.ends.len() - self.ends.binary_search(&end).expect("indexed span end")
    }

    fn insert(&mut self, end: u64, candidate: ParentCandidate) {
        let mut rank = self.rank(end);
        while rank < self.tree.len() {
            self.tree[rank] = merge_candidates(self.tree[rank], Some(candidate));
            rank += rank & rank.wrapping_neg();
        }
    }

    fn best(&self, end: u64) -> Option<ParentCandidate> {
        let mut rank = self.rank(end);
        let mut best = None;
        while rank > 0 {
            best = merge_candidates(best, self.tree[rank]);
            rank &= rank - 1;
        }
        best
    }
}

fn unique_span_ends(spans: &[SpanBuilder]) -> Vec<u64> {
    let mut ends: Vec<_> = spans
        .iter()
        .filter_map(|span| span_bounds(span).map(|(_, end)| end))
        .collect();
    ends.sort_unstable();
    ends.dedup();
    ends
}

fn merge_candidates(
    left: Option<ParentCandidate>,
    right: Option<ParentCandidate>,
) -> Option<ParentCandidate> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.prefer(right)),
        (candidate, None) | (None, candidate) => candidate,
    }
}

fn span_bounds(span: &SpanBuilder) -> Option<(u64, u64)> {
    span_start(span).zip(span_end(span))
}

fn span_sort_key(span: &SpanBuilder) -> (u64, Reverse<u64>) {
    (
        span_start(span).unwrap_or(u64::MAX),
        Reverse(span_end(span).unwrap_or(0)),
    )
}

fn assign_parent(spans: &mut [SpanBuilder], index: usize, parents: &ParentIndex) {
    let Some((_, end)) = span_bounds(&spans[index]) else {
        return;
    };
    let Some(parent) = parents.best(end) else {
        return;
    };
    spans[index].parent_span_id = Some(spans[parent.index].span_id.clone());
    spans[index].depth = parent.depth + 1;
}

fn index_parent(parents: &mut ParentIndex, spans: &[SpanBuilder], index: usize) {
    let Some((_, end)) = span_bounds(&spans[index]) else {
        return;
    };
    parents.insert(
        end,
        ParentCandidate {
            index,
            depth: spans[index].depth,
        },
    );
}

pub(crate) fn assign_parents(spans: &mut [SpanBuilder]) {
    spans.sort_by_key(span_sort_key);
    let mut parents = ParentIndex::new(spans);
    for index in 0..spans.len() {
        assign_parent(spans, index, &parents);
        index_parent(&mut parents, spans, index);
    }
}

fn seed_subtree_totals(span: &mut SpanBuilder) {
    span.subtree_cost_usd_e6 = span.cost_usd_e6;
    span.subtree_token_count = span
        .tokens_in
        .map(|tokens| tokens + span.tokens_out.unwrap_or(0));
}

fn first_span_indices(spans: &[SpanBuilder]) -> HashMap<String, usize> {
    spans
        .iter()
        .enumerate()
        .fold(HashMap::new(), |mut ids, (index, span)| {
            ids.entry(span.span_id.clone()).or_insert(index);
            ids
        })
}

fn descending_depth_indices(spans: &[SpanBuilder]) -> Vec<usize> {
    let mut indices: Vec<_> = (0..spans.len()).collect();
    indices.sort_by_key(|&index| Reverse(spans[index].depth));
    indices
}

fn add_cost(total: &mut Option<i64>, value: Option<i64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0) + value);
    }
}

fn add_tokens(total: &mut Option<u32>, value: Option<u32>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0) + value);
    }
}

fn parent_index(span: &SpanBuilder, ids: &HashMap<String, usize>) -> Option<usize> {
    span.parent_span_id
        .as_ref()
        .and_then(|id| ids.get(id))
        .copied()
}

fn roll_up_span(spans: &mut [SpanBuilder], index: usize, ids: &HashMap<String, usize>) {
    let Some(parent) = parent_index(&spans[index], ids) else {
        return;
    };
    let cost = spans[index].subtree_cost_usd_e6;
    let tokens = spans[index].subtree_token_count;
    add_cost(&mut spans[parent].subtree_cost_usd_e6, cost);
    add_tokens(&mut spans[parent].subtree_token_count, tokens);
}

pub(crate) fn compute_subtree_costs(spans: &mut [SpanBuilder]) {
    spans.iter_mut().for_each(seed_subtree_totals);
    let ids = first_span_indices(spans);
    for index in descending_depth_indices(spans) {
        roll_up_span(spans, index, &ids);
    }
}

#[cfg(test)]
mod tests;
