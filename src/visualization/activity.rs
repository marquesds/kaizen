// SPDX-License-Identifier: AGPL-3.0-or-later

use super::rollup::{counts, first_count, kind_name, token_totals};
use super::types::{ActivityBin, ActivityMetric, ActivityReport};
use crate::core::event::{Event, SessionRecord};
use std::collections::HashSet;

const DAY_MS: u64 = 86_400_000;

pub(super) fn activity(pairs: &[(SessionRecord, Event)], now_ms: u64) -> ActivityReport {
    let mut day_bins = bins(pairs, now_ms.saturating_sub(DAY_MS), now_ms, 5 * 60_000);
    let mut week_bins = bins(
        pairs,
        now_ms.saturating_sub(7 * DAY_MS),
        now_ms,
        30 * 60_000,
    );
    normalize(&mut day_bins);
    normalize(&mut week_bins);
    ActivityReport {
        metric: ActivityMetric::Events,
        day_bins,
        week_bins,
    }
}

fn bins(pairs: &[(SessionRecord, Event)], start: u64, end: u64, width: u64) -> Vec<ActivityBin> {
    (start..end)
        .step_by(width as usize)
        .map(|s| bin(pairs, s, s.saturating_add(width)))
        .collect()
}

fn bin(pairs: &[(SessionRecord, Event)], start: u64, end: u64) -> ActivityBin {
    let scoped: Vec<_> = pairs
        .iter()
        .filter(|(_, e)| e.ts_ms >= start && e.ts_ms < end)
        .collect();
    let agents = counts(scoped.iter().map(|(s, _)| s.agent.as_str()));
    ActivityBin {
        start_ms: start,
        end_ms: end,
        event_count: scoped.len() as u64,
        session_count: session_count(&scoped),
        token_total: token_totals(scoped.iter().map(|(_, e)| e)).total,
        cost_usd_e6: scoped.iter().filter_map(|(_, e)| e.cost_usd_e6).sum(),
        dominant_agent: first_count(&agents),
        dominant_kind: first_count(&counts(scoped.iter().map(|(_, e)| kind_name(&e.kind)))),
        active_by_agent: agents,
        heat: 0.0,
        is_break: scoped.is_empty(),
    }
}

fn normalize(bins: &mut [ActivityBin]) {
    let max = bins.iter().map(|b| b.event_count).max().unwrap_or(0).max(1);
    bins.iter_mut()
        .for_each(|b| b.heat = b.event_count as f64 / max as f64);
}

fn session_count(scoped: &[&(SessionRecord, Event)]) -> u64 {
    scoped
        .iter()
        .map(|(s, _)| s.id.as_str())
        .collect::<HashSet<_>>()
        .len() as u64
}
