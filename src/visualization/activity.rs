// SPDX-License-Identifier: AGPL-3.0-or-later

use super::types::{ActivityBin, ActivityMetric, ActivityReport};
use crate::store::Store;
use anyhow::Result;

const DAY_MS: u64 = 86_400_000;
const DAY_BIN_MS: u64 = 5 * 60_000;
const WEEK_BIN_MS: u64 = 30 * 60_000;

pub(super) fn activity(store: &Store, workspace: &str, now_ms: u64) -> Result<ActivityReport> {
    Ok(ActivityReport {
        metric: ActivityMetric::Events,
        day_bins: bins(
            store,
            workspace,
            now_ms.saturating_sub(DAY_MS),
            now_ms,
            DAY_BIN_MS,
        )?,
        week_bins: bins(
            store,
            workspace,
            now_ms.saturating_sub(7 * DAY_MS),
            now_ms,
            WEEK_BIN_MS,
        )?,
    })
}

fn bins(
    store: &Store,
    workspace: &str,
    start: u64,
    end: u64,
    width: u64,
) -> Result<Vec<ActivityBin>> {
    let read = store.visualization_activity(workspace, start, end, width)?;
    let mut bins = states(start, end, width);
    read.totals.into_iter().for_each(|row| {
        set_total(
            &mut bins,
            row.bin,
            row.event_count,
            row.session_count,
            row.token_total,
            row.cost_usd_e6,
        )
    });
    read.agents
        .into_iter()
        .for_each(|row| push_agent(&mut bins, row.bin, row.name, row.count));
    let mut kinds = vec![Vec::new(); bins.len()];
    read.kinds
        .into_iter()
        .for_each(|row| push_count(&mut kinds, row.bin, row.name, row.count));
    finish(&mut bins, kinds);
    Ok(bins)
}

fn states(start: u64, end: u64, width: u64) -> Vec<ActivityBin> {
    (start..end)
        .step_by(width as usize)
        .map(|start_ms| ActivityBin {
            start_ms,
            end_ms: start_ms.saturating_add(width),
            is_break: true,
            ..Default::default()
        })
        .collect()
}

fn set_total(
    bins: &mut [ActivityBin],
    index: usize,
    events: u64,
    sessions: u64,
    tokens: u64,
    cost: i64,
) {
    let Some(bin) = bins.get_mut(index) else {
        return;
    };
    bin.event_count = events;
    bin.session_count = sessions;
    bin.token_total = tokens;
    bin.cost_usd_e6 = cost;
    bin.is_break = false;
}

fn push_agent(bins: &mut [ActivityBin], index: usize, name: String, count: u64) {
    if let Some(bin) = bins.get_mut(index) {
        bin.active_by_agent.push((name, count));
    }
}

fn push_count(counts: &mut [Vec<(String, u64)>], index: usize, name: String, count: u64) {
    if let Some(bin) = counts.get_mut(index) {
        bin.push((name, count));
    }
}

fn finish(bins: &mut [ActivityBin], kinds: Vec<Vec<(String, u64)>>) {
    let max = bins
        .iter()
        .map(|bin| bin.event_count)
        .max()
        .unwrap_or(0)
        .max(1);
    bins.iter_mut().zip(kinds).for_each(|(bin, kinds)| {
        bin.dominant_agent = bin.active_by_agent.first().map(|row| row.0.clone());
        bin.dominant_kind = kinds.first().map(|row| row.0.clone());
        bin.heat = bin.event_count as f64 / max as f64;
    });
}
