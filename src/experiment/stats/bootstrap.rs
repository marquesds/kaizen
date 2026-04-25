// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bootstrap CI and winsorization helpers.
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

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

/// 95% percentile bootstrap CI on the median delta (treatment − control).
pub fn bootstrap_ci(
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

/// Block-bootstrap CI where each element of `clusters_*` is one cluster's values.
///
/// Resamples whole clusters with replacement so within-cluster correlation
/// doesn't inflate precision. Falls back to point-wise bootstrap when clusters
/// are singletons (one session per cluster).
pub fn cluster_bootstrap_ci(
    clusters_control: &[Vec<f64>],
    clusters_treatment: &[Vec<f64>],
    seed: u64,
    resamples: u32,
) -> (Option<f64>, Option<f64>) {
    if clusters_control.is_empty() || clusters_treatment.is_empty() {
        return (None, None);
    }
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut deltas: Vec<f64> = Vec::with_capacity(resamples as usize);
    for _ in 0..resamples {
        let sample_c: Vec<f64> = (0..clusters_control.len())
            .flat_map(|_| {
                let idx = rng.random_range(0..clusters_control.len());
                clusters_control[idx].iter().copied()
            })
            .collect();
        let sample_t: Vec<f64> = (0..clusters_treatment.len())
            .flat_map(|_| {
                let idx = rng.random_range(0..clusters_treatment.len());
                clusters_treatment[idx].iter().copied()
            })
            .collect();
        let (Some(mc), Some(mt)) = (median(&sample_c), median(&sample_t)) else {
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

pub fn quantile(xs: &[f64], p: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((v.len() - 1) as f64 * p).round() as usize;
    Some(v[idx.min(v.len() - 1)])
}

pub fn median(xs: &[f64]) -> Option<f64> {
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

pub fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    Some(xs.iter().sum::<f64>() / xs.len() as f64)
}
