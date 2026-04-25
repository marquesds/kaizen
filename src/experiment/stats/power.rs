// SPDX-License-Identifier: AGPL-3.0-or-later
//! Minimum Detectable Effect (MDE) at 80% power / 95% CI.
//!
//! Analytical formula for a two-sample test assuming normal approximation
//! (reasonable for n ≥ 30). Use as a sizing guide, not a guarantee.

/// z-score for α = 0.05 (two-tailed).
const Z_ALPHA: f64 = 1.96;
/// z-score for 80% power.
const Z_BETA: f64 = 0.84;

#[derive(Debug)]
pub struct PowerResult {
    /// Smallest detectable absolute effect at 80% power.
    pub mde_absolute: f64,
    /// MDE as a percentage of the baseline mean.
    pub mde_pct: Option<f64>,
    /// Estimated standard deviation of the metric.
    pub sigma: f64,
    /// Sample size per arm used in the calculation.
    pub n_per_arm: usize,
}

/// Compute MDE given `n_per_arm` and observed metric values.
///
/// Uses `values` to estimate σ. Returns `None` when `values` is empty or
/// `n_per_arm` is zero.
pub fn mde(values: &[f64], n_per_arm: usize) -> Option<PowerResult> {
    if values.is_empty() || n_per_arm == 0 {
        return None;
    }
    let sigma = std_dev(values);
    let baseline_mean = values.iter().sum::<f64>() / values.len() as f64;
    let mde_absolute = (Z_ALPHA + Z_BETA) * sigma * (2.0_f64 / n_per_arm as f64).sqrt();
    let mde_pct = if baseline_mean.abs() > f64::EPSILON {
        Some(100.0 * mde_absolute / baseline_mean.abs())
    } else {
        None
    };
    Some(PowerResult {
        mde_absolute,
        mde_pct,
        sigma,
        n_per_arm,
    })
}

fn std_dev(xs: &[f64]) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let mean = xs.iter().sum::<f64>() / xs.len() as f64;
    let var = xs.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    var.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mde_shrinks_with_larger_n() {
        let values: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let r50 = mde(&values, 50).unwrap();
        let r200 = mde(&values, 200).unwrap();
        assert!(
            r200.mde_absolute < r50.mde_absolute,
            "larger n → smaller MDE"
        );
    }

    #[test]
    fn mde_none_on_empty() {
        assert!(mde(&[], 100).is_none());
    }

    #[test]
    fn mde_none_on_zero_n() {
        assert!(mde(&[1.0, 2.0], 0).is_none());
    }
}
