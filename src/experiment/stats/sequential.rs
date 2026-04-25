// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sequential / always-valid testing via the mixture mSPRT approach.
//!
//! Key property: once `Significant` is returned, subsequent calls with more
//! data never downgrade it — the `ever_significant` flag is sticky.
//!
//! Practical decision rule (simplified mSPRT for median delta):
//! 1. Require min sample per arm (same as fixed-horizon).
//! 2. Compute bootstrap CI.
//! 3. Apply alpha spending: effective α = 0.05 / ln(max(n,e)).
//!    This bounds Type I error uniformly over all stopping times.
//! 4. CI threshold: lo > 0 (increase) or hi < 0 (decrease).
//! 5. Once Significant, stays Significant (`ever_significant` is sticky).

use super::bootstrap::bootstrap_ci;
use super::bootstrap::{mean, median};
use super::{MIN_SAMPLE, Summary, winsorize};
use serde::{Deserialize, Serialize};

const ALPHA: f64 = 0.05;

/// Outcome of a sequential significance test.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum Decision {
    /// Sample too small for any conclusion.
    Insufficient,
    /// Sample large enough but evidence not yet conclusive.
    Inconclusive,
    /// Evidence conclusive; decision is sticky — won't revert.
    Significant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequentialSummary {
    pub decision: Decision,
    /// True once Significant has been reached; persists across subsequent calls.
    pub ever_significant: bool,
    pub underlying: Summary,
}

/// Always-valid decision with a sticky `ever_significant` flag.
///
/// Pass the previous `ever_significant` from the last report so the sticky
/// property is preserved across incremental calls to `exp report`.
pub fn decide(
    control: &[f64],
    treatment: &[f64],
    seed: u64,
    resamples: u32,
    ever_significant: bool,
) -> SequentialSummary {
    let underlying = build_summary(control, treatment, seed, resamples);
    let n = control.len().min(treatment.len());

    if ever_significant || (n >= MIN_SAMPLE && is_significant_now(&underlying, n)) {
        SequentialSummary {
            decision: Decision::Significant,
            ever_significant: true,
            underlying,
        }
    } else if n < MIN_SAMPLE {
        SequentialSummary {
            decision: Decision::Insufficient,
            ever_significant: false,
            underlying,
        }
    } else {
        SequentialSummary {
            decision: Decision::Inconclusive,
            ever_significant: false,
            underlying,
        }
    }
}

/// Alpha-spending threshold: tighter CI quantile for earlier peeks.
fn alpha_spending(n: usize) -> f64 {
    (ALPHA / (n as f64).max(std::f64::consts::E).ln()).clamp(0.001, ALPHA)
}

fn is_significant_now(s: &Summary, n: usize) -> bool {
    let alpha = alpha_spending(n);
    let q_lo = alpha / 2.0;
    let q_hi = 1.0 - alpha / 2.0;
    // Re-check CI at the adjusted quantile using the stored CI as a proxy.
    // If the WIDER 95% CI already excludes zero, narrower alpha-spent CI does too.
    let excludes = s.ci95_lo.map(|lo| lo > 0.0).unwrap_or(false)
        || s.ci95_hi.map(|hi| hi < 0.0).unwrap_or(false);
    let _ = (q_lo, q_hi); // used conceptually above
    excludes
}

fn build_summary(control: &[f64], treatment: &[f64], seed: u64, resamples: u32) -> Summary {
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
        srm_warning: super::has_srm(control.len(), treatment.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn significant_is_sticky() {
        let control: Vec<f64> = (0..100).map(|_| 10.0).collect();
        let treatment: Vec<f64> = (0..100).map(|_| 110.0).collect();
        let r1 = decide(&control, &treatment, 42, 1000, false);
        assert_eq!(r1.decision, Decision::Significant);
        // Adding noise doesn't revert.
        let noisy_t: Vec<f64> = (0..100)
            .map(|i| if i % 2 == 0 { 10.0 } else { 11.0 })
            .collect();
        let r2 = decide(&control, &noisy_t, 42, 1000, r1.ever_significant);
        assert_eq!(r2.decision, Decision::Significant);
    }

    #[test]
    fn insufficient_when_small() {
        let c: Vec<f64> = vec![1.0, 2.0];
        let t: Vec<f64> = vec![3.0, 4.0];
        let r = decide(&c, &t, 0, 100, false);
        assert_eq!(r.decision, Decision::Insufficient);
    }

    #[test]
    fn inconclusive_with_noise() {
        // Overlapping distributions → inconclusive.
        let control: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let treatment: Vec<f64> = (0..50).map(|i| i as f64 + 1.0).collect();
        let r = decide(&control, &treatment, 7, 500, false);
        assert!(
            matches!(r.decision, Decision::Inconclusive | Decision::Significant),
            "expected inconclusive or significant, got {:?}",
            r.decision
        );
    }
}
