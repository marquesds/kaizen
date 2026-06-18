// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

fn span(id: &str, start: u64, end: u64) -> SpanBuilder {
    SpanBuilder {
        span_id: id.to_owned(),
        hook_start_ms: Some(start),
        hook_end_ms: Some(end),
        ..Default::default()
    }
}

fn hierarchy<'a>(spans: &'a [SpanBuilder], id: &str) -> (Option<&'a str>, u32) {
    let span = spans.iter().find(|span| span.span_id == id).unwrap();
    (span.parent_span_id.as_deref(), span.depth)
}

fn totals(spans: &[SpanBuilder], id: &str) -> (Option<i64>, Option<u32>) {
    let span = spans.iter().find(|span| span.span_id == id).unwrap();
    (span.subtree_cost_usd_e6, span.subtree_token_count)
}

#[test]
fn parent_index_prefers_depth_then_earliest_input() {
    let spans = [
        span("early", 0, 10),
        span("late", 1, 11),
        span("deep", 2, 9),
        span("child", 3, 8),
    ];
    let mut index = ParentIndex::new(&spans);
    index.insert(10, ParentCandidate { index: 0, depth: 0 });
    index.insert(11, ParentCandidate { index: 1, depth: 0 });
    index.insert(9, ParentCandidate { index: 2, depth: 1 });
    assert_eq!(index.best(8).map(|candidate| candidate.index), Some(2));
    assert_eq!(index.best(10).map(|candidate| candidate.index), Some(0));
}

#[test]
fn assign_parents_preserves_inclusive_deepest_and_tie_rules() {
    let mut spans = vec![
        span("outer", 0, 10),
        span("crossing", 1, 5),
        span("overlap", 2, 8),
        span("leaf", 3, 4),
        span("same-a", 20, 30),
        span("same-b", 20, 30),
        span("same-c", 20, 30),
        span("tie-a", 40, 50),
        span("tie-b", 41, 51),
        span("tie-child", 42, 49),
    ];
    assign_parents(&mut spans);
    assert_eq!(hierarchy(&spans, "leaf"), (Some("crossing"), 2));
    assert_eq!(hierarchy(&spans, "same-b"), (Some("same-a"), 1));
    assert_eq!(hierarchy(&spans, "same-c"), (Some("same-b"), 2));
    assert_eq!(hierarchy(&spans, "tie-child"), (Some("tie-a"), 1));
}

#[test]
fn assign_parents_ignores_incomplete_spans() {
    let mut incomplete = span("incomplete", 0, 10);
    incomplete.hook_end_ms = None;
    incomplete.depth = 99;
    let mut spans = vec![incomplete, span("complete", 1, 2)];
    assign_parents(&mut spans);
    assert_eq!(hierarchy(&spans, "complete"), (None, 0));
}

#[test]
fn compute_subtree_costs_rolls_descendants_into_ancestors() {
    let mut root = span("root", 0, 10);
    root.cost_usd_e6 = Some(1);
    root.tokens_in = Some(1);
    root.tokens_out = Some(2);
    let mut child = span("child", 1, 8);
    child.parent_span_id = Some("root".to_owned());
    child.depth = 1;
    child.cost_usd_e6 = Some(2);
    child.tokens_in = Some(4);
    child.tokens_out = Some(5);
    let mut grandchild = span("grandchild", 2, 3);
    grandchild.parent_span_id = Some("child".to_owned());
    grandchild.depth = 2;
    grandchild.cost_usd_e6 = Some(3);
    grandchild.tokens_out = Some(7);
    let mut sibling = span("sibling", 8, 9);
    sibling.parent_span_id = Some("root".to_owned());
    sibling.depth = 1;
    sibling.tokens_in = Some(2);
    let mut spans = vec![root, child, grandchild, sibling];
    compute_subtree_costs(&mut spans);
    assert_eq!(totals(&spans, "root"), (Some(6), Some(14)));
    assert_eq!(totals(&spans, "child"), (Some(5), Some(9)));
    assert_eq!(totals(&spans, "grandchild"), (Some(3), None));
    assert_eq!(totals(&spans, "sibling"), (None, Some(2)));
}

#[test]
fn compute_subtree_costs_uses_first_duplicate_parent_id() {
    let mut first = span("parent", 0, 10);
    first.cost_usd_e6 = Some(1);
    let mut duplicate = span("parent", 20, 30);
    duplicate.cost_usd_e6 = Some(10);
    let mut child = span("child", 1, 2);
    child.parent_span_id = Some("parent".to_owned());
    child.depth = 1;
    child.cost_usd_e6 = Some(2);
    let mut spans = vec![first, duplicate, child];
    compute_subtree_costs(&mut spans);
    assert_eq!(spans[0].subtree_cost_usd_e6, Some(3));
    assert_eq!(spans[1].subtree_cost_usd_e6, Some(10));
}
