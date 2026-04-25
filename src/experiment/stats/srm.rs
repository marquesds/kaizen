// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sample Ratio Mismatch (SRM) detection.
//!
//! Chi-squared test on observed arm counts. If the experiment expects an
//! even 50/50 split but the observed ratio deviates significantly
//! (χ²(1) > 10.83, p < 0.001), something is wrong with routing or selection.

/// χ²(1) critical value for p = 0.001.
const SRM_CHI2_THRESHOLD: f64 = 10.83;

/// Chi-squared statistic for a 50/50 expected split.
pub fn chi2(n_control: usize, n_treatment: usize) -> Option<f64> {
    let total = n_control + n_treatment;
    if total == 0 {
        return None;
    }
    let expected = total as f64 / 2.0;
    let stat = (n_control as f64 - expected).powi(2) / expected
        + (n_treatment as f64 - expected).powi(2) / expected;
    Some(stat)
}

/// Returns `true` when the arm counts show a sample ratio mismatch at p < 0.001.
pub fn has_srm(n_control: usize, n_treatment: usize) -> bool {
    chi2(n_control, n_treatment)
        .map(|c| c > SRM_CHI2_THRESHOLD)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_arms_no_srm() {
        assert!(!has_srm(500, 500));
        assert!(!has_srm(100, 100));
    }

    #[test]
    fn severe_imbalance_flags_srm() {
        // 800 vs 200 out of 1000 → extreme imbalance
        assert!(has_srm(800, 200));
    }

    #[test]
    fn empty_arms_no_srm() {
        assert!(!has_srm(0, 0));
    }
}
