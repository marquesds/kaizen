// SPDX-License-Identifier: AGPL-3.0-or-later
//! Non-parametric stats for experiment reports.
//!
//! Effect size = median(treatment) − median(control). CI via
//! percentile bootstrap (default 10k resamples, 95%). Winsorize p1/p99
//! before resampling to blunt skew.

use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

pub const DEFAULT_RESAMPLES: u32 = 10_000;
pub const MIN_SAMPLE: usize = 30;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub n_control: usize,
    pub n_treatment: usize,
    pub median_control: Option<f64>,
    pub median_treatment: Option<f64>,
    pub mean_control: Option<f64>,
    pub mean_treatment: Option<f64>,
    pub delta_median: Option<f64>,
    pub delta_pct: Option<f64>,
    pub ci95_lo: Option<f64>,
    pub ci95_hi: Option<f64>,
    pub small_sample_warning: bool,
}

/// Pure stats for a metric. Deterministic given `seed`.
pub fn summarize(control: &[f64], treatment: &[f64], seed: u64, resamples: u32) -> Summary {
    let c = winsorize(control, 0.01, 0.99);
    let t = winsorize(treatment, 0.01, 0.99);
    let median_c = median(&c);
    let median_t = median(&t);
    let mean_c = mean(&c);
    let mean_t = mean(&t);
    let delta = match (median_c, median_t) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    };
    let delta_pct = match (median_c, delta) {
        (Some(a), Some(d)) if a != 0.0 => Some(100.0 * d / a),
        _ => None,
    };
    let (lo, hi) = if c.is_empty() || t.is_empty() {
        (None, None)
    } else {
        bootstrap_ci(&c, &t, seed, resamples)
    };
    Summary {
        n_control: control.len(),
        n_treatment: treatment.len(),
        median_control: median_c,
        median_treatment: median_t,
        mean_control: mean_c,
        mean_treatment: mean_t,
        delta_median: delta,
        delta_pct,
        ci95_lo: lo,
        ci95_hi: hi,
        small_sample_warning: control.len().min(treatment.len()) < MIN_SAMPLE,
    }
}

/// Clamp values to `[p_lo quantile, p_hi quantile]`.
pub fn winsorize(xs: &[f64], p_lo: f64, p_hi: f64) -> Vec<f64> {
    if xs.is_empty() {
        return Vec::new();
    }
    let Some(lo) = quantile(xs, p_lo) else {
        return xs.to_vec();
    };
    let Some(hi) = quantile(xs, p_hi) else {
        return xs.to_vec();
    };
    xs.iter().map(|v| v.clamp(lo, hi)).collect()
}

fn quantile(xs: &[f64], p: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((v.len() - 1) as f64 * p).round() as usize;
    Some(v[idx.min(v.len() - 1)])
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        Some(v[n / 2])
    } else {
        Some((v[n / 2 - 1] + v[n / 2]) / 2.0)
    }
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    Some(xs.iter().sum::<f64>() / xs.len() as f64)
}

fn bootstrap_ci(
    control: &[f64],
    treatment: &[f64],
    seed: u64,
    resamples: u32,
) -> (Option<f64>, Option<f64>) {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut deltas: Vec<f64> = Vec::with_capacity(resamples as usize);
    let mut buf_c = vec![0.0_f64; control.len()];
    let mut buf_t = vec![0.0_f64; treatment.len()];
    for _ in 0..resamples {
        for slot in buf_c.iter_mut() {
            *slot = control[rng.random_range(0..control.len())];
        }
        for slot in buf_t.iter_mut() {
            *slot = treatment[rng.random_range(0..treatment.len())];
        }
        let (Some(mc), Some(mt)) = (median(&buf_c), median(&buf_t)) else {
            continue;
        };
        deltas.push(mt - mc);
    }
    if deltas.is_empty() {
        return (None, None);
    }
    deltas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lo_i = ((deltas.len() as f64 * 0.025).round() as usize).min(deltas.len() - 1);
    let hi_i = ((deltas.len() as f64 * 0.975).round() as usize).min(deltas.len() - 1);
    (Some(deltas[lo_i]), Some(deltas[hi_i]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_positive_shift_detected() {
        // Two tight clusters separated by 100 — CI must clear 0 comfortably.
        let control: Vec<f64> = (0..100).map(|_| 10.0).collect();
        let treatment: Vec<f64> = (0..100).map(|_| 110.0).collect();
        let s = summarize(&control, &treatment, 42, 1000);
        assert_eq!(s.delta_median, Some(100.0));
        let lo = s.ci95_lo.unwrap();
        let hi = s.ci95_hi.unwrap();
        assert!(lo > 0.0, "CI should exclude zero above, got {lo}");
        assert!(hi >= lo);
    }

    #[test]
    fn small_sample_warns() {
        let c: Vec<f64> = vec![1.0, 2.0, 3.0];
        let t: Vec<f64> = vec![4.0, 5.0, 6.0];
        let s = summarize(&c, &t, 1, 100);
        assert!(s.small_sample_warning);
    }

    #[test]
    fn winsorize_clips_outliers() {
        // 200 ordinary values + one extreme; p99 quantile ignores the tail.
        let mut xs: Vec<f64> = (0..200).map(|i| i as f64).collect();
        xs.push(10_000.0);
        let w = winsorize(&xs, 0.01, 0.99);
        let max_w = w.iter().cloned().fold(f64::MIN, f64::max);
        assert!(max_w < 10_000.0, "extreme still present: {max_w}");
    }

    #[test]
    fn empty_inputs_safe() {
        let s = summarize(&[], &[], 0, 10);
        assert_eq!(s.n_control, 0);
        assert!(s.delta_median.is_none());
        assert!(s.ci95_lo.is_none());
    }
}
