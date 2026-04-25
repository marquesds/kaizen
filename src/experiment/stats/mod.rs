// SPDX-License-Identifier: AGPL-3.0-or-later
//! Non-parametric stats for experiment reports.
//!
//! Effect size = median(treatment) − median(control). CI via
//! percentile bootstrap (default 10k resamples, 95%). Winsorize p1/p99
//! before resampling to blunt skew.

pub mod bootstrap;
pub mod cuped;
pub mod power;
pub mod sequential;
pub mod srm;

pub use bootstrap::winsorize;
pub use srm::has_srm;

use bootstrap::{bootstrap_ci, mean, median};
use serde::{Deserialize, Serialize};

pub const DEFAULT_RESAMPLES: u32 = 10_000;
pub const MIN_SAMPLE: usize = 30;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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
    /// Set when observed arm counts deviate from expected 50/50 at p < 0.001.
    pub srm_warning: bool,
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
        srm_warning: has_srm(control.len(), treatment.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_positive_shift_detected() {
        let control: Vec<f64> = (0..100).map(|_| 10.0).collect();
        let treatment: Vec<f64> = (0..100).map(|_| 110.0).collect();
        let s = summarize(&control, &treatment, 42, 1000);
        assert_eq!(s.delta_median, Some(100.0));
        let lo = s.ci95_lo.unwrap();
        let hi = s.ci95_hi.unwrap();
        assert!(lo > 0.0, "CI should exclude zero above, got {lo}");
        assert!(hi >= lo);
        assert!(!s.srm_warning);
    }

    #[test]
    fn srm_warning_on_imbalance() {
        let control: Vec<f64> = (0..800).map(|_| 1.0).collect();
        let treatment: Vec<f64> = (0..200).map(|_| 1.0).collect();
        let s = summarize(&control, &treatment, 0, 100);
        assert!(s.srm_warning, "should flag SRM for 800:200 split");
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
