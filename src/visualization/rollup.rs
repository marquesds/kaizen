// SPDX-License-Identifier: AGPL-3.0-or-later

use super::types::TokenTotals;
use crate::core::event::{Event, EventKind, SessionRecord};
use std::collections::{BTreeMap, HashSet};

pub(super) fn token_totals<'a>(events: impl Iterator<Item = &'a Event>) -> TokenTotals {
    events.fold(TokenTotals::default(), add_tokens).with_total()
}

pub(super) fn counts<'a>(values: impl Iterator<Item = &'a str>) -> Vec<(String, u64)> {
    let mut map = BTreeMap::<String, u64>::new();
    values.for_each(|v| *map.entry(v.to_string()).or_default() += 1);
    sorted_counts(map)
}

pub(super) fn first_count(counts: &[(String, u64)]) -> Option<String> {
    counts.first().map(|(name, _)| name.clone())
}

pub(super) fn kind_name(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::ToolCall => "tool_call",
        EventKind::ToolResult => "tool_result",
        EventKind::Message => "message",
        EventKind::Error => "error",
        EventKind::Cost => "cost",
        EventKind::Hook => "hook",
        EventKind::Lifecycle => "lifecycle",
    }
}

pub(super) fn has_tokens(e: &Event) -> bool {
    e.tokens_in
        .or(e.tokens_out)
        .or(e.reasoning_tokens)
        .or(e.cache_read_tokens)
        .or(e.cache_creation_tokens)
        .is_some()
}

pub(super) fn cost_session_count(pairs: &[(SessionRecord, Event)]) -> usize {
    pairs
        .iter()
        .filter(|(_, e)| e.cost_usd_e6.is_some())
        .map(|(s, _)| s.id.as_str())
        .collect::<HashSet<_>>()
        .len()
}

pub(super) fn pct(total: usize, count: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 * 100.0 / total as f64
    }
}

fn add_tokens(mut out: TokenTotals, event: &Event) -> TokenTotals {
    out.input += event.tokens_in.unwrap_or(0) as u64;
    out.output += event.tokens_out.unwrap_or(0) as u64;
    out.reasoning += event.reasoning_tokens.unwrap_or(0) as u64;
    out.cache_read += event.cache_read_tokens.unwrap_or(0) as u64;
    out.cache_create += event.cache_creation_tokens.unwrap_or(0) as u64;
    out
}

fn sorted_counts(map: BTreeMap<String, u64>) -> Vec<(String, u64)> {
    let mut out: Vec<_> = map.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

impl TokenTotals {
    fn with_total(mut self) -> Self {
        self.total =
            self.input + self.output + self.reasoning + self.cache_read + self.cache_create;
        self
    }
}
