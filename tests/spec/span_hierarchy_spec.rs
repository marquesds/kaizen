// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct QuintSpan {
    id: i64,
    start: i64,
    end: i64,
    parent: i64,
    depth: i64,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct SpanHierarchyState {
    spans: Vec<QuintSpan>,
    next_id: i64,
}

#[derive(Debug, Default)]
struct SpanHierarchyDriver {
    spans: Vec<QuintSpan>,
    next_id: i64,
}

fn contains(outer: &QuintSpan, inner: &QuintSpan) -> bool {
    outer.start <= inner.start && inner.end <= outer.end && outer.id != inner.id
}

fn assign_parent(candidate_start: i64, candidate_end: i64, id: i64, spans: &[QuintSpan]) -> i64 {
    let probe = QuintSpan { id, start: candidate_start, end: candidate_end, parent: -1, depth: 0 };
    spans
        .iter()
        .filter(|s| contains(s, &probe))
        .max_by_key(|s| s.depth)
        .map(|s| s.id)
        .unwrap_or(-1)
}

fn depth_of(pid: i64, spans: &[QuintSpan]) -> i64 {
    if pid == -1 {
        return 0;
    }
    spans.iter().find(|s| s.id == pid).map(|p| p.depth + 1).unwrap_or(0)
}

impl State<SpanHierarchyDriver> for SpanHierarchyState {
    fn from_driver(d: &SpanHierarchyDriver) -> Result<Self> {
        Ok(SpanHierarchyState {
            spans: d.spans.clone(),
            next_id: d.next_id,
        })
    }
}

impl Driver for SpanHierarchyDriver {
    type State = SpanHierarchyState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.spans = vec![];
                self.next_id = 0;
            },
            step => {},
            add_root_span => {
                let s = QuintSpan { id: self.next_id, start: 0, end: 100, parent: -1, depth: 0 };
                self.spans.push(s);
                self.next_id += 1;
            },
            add_child_span => {
                if !self.spans.is_empty() {
                    let pid = assign_parent(10, 50, self.next_id, &self.spans);
                    let d = depth_of(pid, &self.spans);
                    let s = QuintSpan { id: self.next_id, start: 10, end: 50, parent: pid, depth: d };
                    self.spans.push(s);
                    self.next_id += 1;
                }
            },
            add_grandchild_span => {
                if self.spans.len() >= 2 {
                    let pid = assign_parent(15, 30, self.next_id, &self.spans);
                    let d = depth_of(pid, &self.spans);
                    let s = QuintSpan { id: self.next_id, start: 15, end: 30, parent: pid, depth: d };
                    self.spans.push(s);
                    self.next_id += 1;
                }
            },
        })
    }
}

#[quint_run(spec = "specs/span-hierarchy.qnt", max_samples = 20, max_steps = 10)]
fn span_hierarchy_run() -> impl Driver {
    SpanHierarchyDriver::default()
}
