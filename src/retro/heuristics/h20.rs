// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::event::EventSource;
use crate::retro::types::{Bet, Inputs};

const MIN_CALLS: usize = 20;
const MIN_CACHE_HIT_RATIO: f64 = 0.2;

pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let (total_calls, total_in, total_cache_read) = sum_proxy_cache(inputs);
    if total_calls < MIN_CALLS {
        return vec![];
    }
    let denominator = total_in + total_cache_read;
    if denominator == 0 {
        return vec![];
    }
    let hit_ratio = total_cache_read as f64 / denominator as f64;
    if hit_ratio >= MIN_CACHE_HIT_RATIO {
        return vec![];
    }
    let savings = (MIN_CACHE_HIT_RATIO - hit_ratio) * total_in as f64 * 0.1;
    vec![Bet {
        id: format!("H20:cache_hit:{:.2}", hit_ratio),
        heuristic_id: "H20".into(),
        title: format!(
            "Low Anthropic cache hit ratio ({:.0}% over {total_calls} calls)",
            hit_ratio * 100.0
        ),
        hypothesis: format!(
            "Cache hit ratio {:.1}% < {:.0}%. Unstable system prompt prevents prefix caching.",
            hit_ratio * 100.0,
            MIN_CACHE_HIT_RATIO * 100.0
        ),
        expected_tokens_saved_per_week: savings,
        effort_minutes: 60,
        evidence: vec![format!(
            "{total_calls} proxy calls, cache_read={total_cache_read}, tokens_in={total_in}"
        )],
        apply_step: "Stabilize the system prompt and promote it to a cached prefix block.".into(),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}

fn sum_proxy_cache(inputs: &Inputs) -> (usize, u64, u64) {
    let mut calls = 0usize;
    let mut total_in = 0u64;
    let mut total_cache_read = 0u64;
    for (_, event) in &inputs.events {
        if event.source != EventSource::Proxy {
            continue;
        }
        let (Some(cache_read), Some(tin)) = (event.cache_read_tokens, event.tokens_in) else {
            continue;
        };
        calls += 1;
        total_in += tin as u64;
        total_cache_read += cache_read as u64;
    }
    (calls, total_in, total_cache_read)
}
