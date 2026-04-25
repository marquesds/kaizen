// SPDX-License-Identifier: AGPL-3.0-or-later
//! CUPED — Controlled-experiment Using Pre-Experiment Data.
//!
//! Regresses out pre-period variance to reduce CI width by ~30–50%.
//! Precondition: `values` and `pre_values` must be aligned (same unit, same order).
//! If `pre_values` is empty or has no variance, returns `values` unchanged.

/// Compute the regression coefficient θ = Cov(Y, Y_pre) / Var(Y_pre).
fn theta(values: &[f64], pre_values: &[f64]) -> f64 {
    if values.len() != pre_values.len() || values.is_empty() {
        return 0.0;
    }
    let mean_v = values.iter().sum::<f64>() / values.len() as f64;
    let mean_p = pre_values.iter().sum::<f64>() / pre_values.len() as f64;
    let cov: f64 = values
        .iter()
        .zip(pre_values)
        .map(|(v, p)| (v - mean_v) * (p - mean_p))
        .sum::<f64>()
        / values.len() as f64;
    let var_p: f64 =
        pre_values.iter().map(|p| (p - mean_p).powi(2)).sum::<f64>() / pre_values.len() as f64;
    if var_p < f64::EPSILON {
        return 0.0;
    }
    cov / var_p
}

/// Apply CUPED adjustment: Y_adj = Y − θ * (Y_pre − mean(Y_pre)).
///
/// Returns `values` unchanged when `pre_values` is empty or mismatched.
pub fn adjust(values: &[f64], pre_values: &[f64]) -> Vec<f64> {
    if values.len() != pre_values.len() || values.is_empty() {
        return values.to_vec();
    }
    let t = theta(values, pre_values);
    let mean_p = pre_values.iter().sum::<f64>() / pre_values.len() as f64;
    values
        .iter()
        .zip(pre_values)
        .map(|(v, p)| v - t * (p - mean_p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjustment_reduces_variance() {
        // Y = 2*Y_pre + noise; CUPED should remove the Y_pre component.
        let pre: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let values: Vec<f64> = pre.iter().map(|p| 2.0 * p + 5.0).collect();
        let adjusted = adjust(&values, &pre);
        // After adjustment, variance should be near zero.
        let mean_adj = adjusted.iter().sum::<f64>() / adjusted.len() as f64;
        let var_adj =
            adjusted.iter().map(|v| (v - mean_adj).powi(2)).sum::<f64>() / adjusted.len() as f64;
        assert!(
            var_adj < 1.0,
            "CUPED should collapse variance; got {var_adj}"
        );
    }

    #[test]
    fn mismatched_lengths_returns_original() {
        let v = vec![1.0, 2.0, 3.0];
        let p = vec![1.0, 2.0];
        assert_eq!(adjust(&v, &p), v);
    }

    #[test]
    fn empty_pre_returns_original() {
        let v = vec![1.0, 2.0, 3.0];
        assert_eq!(adjust(&v, &[]), v);
    }
}
