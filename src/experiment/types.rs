// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure data for experiments. See `docs/experiments.md`.

use serde::{Deserialize, Serialize};

/// Variant a session falls into under a binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Classification {
    Control,
    Treatment,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metric {
    TokensPerSession,
    CostPerSession,
    SuccessRate,
    ToolLoops,
    DurationMinutes,
    FilesPerSession,
    SuccessRateByPrompt,
    CostByPrompt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Binding {
    GitCommit {
        control_commit: String,
        treatment_commit: String,
    },
    Branch {
        control_branch: String,
        treatment_branch: String,
    },
    ManualTag {
        variant_field: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Decrease,
    Increase,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Criterion {
    Delta {
        direction: Direction,
        target_pct: f64,
    },
    Absolute {
        metric_value: f64,
    },
}

/// Lifecycle state. `Archived` is terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Draft,
    Running,
    Concluded,
    Archived,
}

/// Guardrail: a secondary metric that must not regress.
///
/// If the CI shows a regression beyond `threshold_pct` in the specified
/// direction, the report flags the guardrail as violated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardrailSpec {
    pub metric: Metric,
    /// Direction of *regression* (e.g. `Increase` for cost = cost going up is bad).
    pub regression_direction: Direction,
    /// Flag if CI endpoint crosses this threshold.
    pub threshold_pct: f64,
}

/// Per-guardrail result in the experiment report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuardrailResult {
    pub metric: Metric,
    pub delta_pct: Option<f64>,
    pub violated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Experiment {
    pub id: String,
    pub name: String,
    pub hypothesis: String,
    pub change_description: String,
    pub metric: Metric,
    pub binding: Binding,
    pub duration_days: u32,
    pub success_criterion: Criterion,
    pub state: State,
    pub created_at_ms: u64,
    pub concluded_at_ms: Option<u64>,
    #[serde(default)]
    pub guardrails: Vec<GuardrailSpec>,
}

impl Metric {
    pub fn as_str(&self) -> &'static str {
        match self {
            Metric::TokensPerSession => "tokens_per_session",
            Metric::CostPerSession => "cost_per_session",
            Metric::SuccessRate => "success_rate",
            Metric::ToolLoops => "tool_loops",
            Metric::DurationMinutes => "duration_minutes",
            Metric::FilesPerSession => "files_per_session",
            Metric::SuccessRateByPrompt => "success_rate_by_prompt",
            Metric::CostByPrompt => "cost_by_prompt",
        }
    }

    pub fn parse(s: &str) -> Option<Metric> {
        Some(match s {
            "tokens_per_session" => Metric::TokensPerSession,
            "cost_per_session" => Metric::CostPerSession,
            "success_rate" => Metric::SuccessRate,
            "tool_loops" => Metric::ToolLoops,
            "duration_minutes" => Metric::DurationMinutes,
            "files_per_session" => Metric::FilesPerSession,
            "success_rate_by_prompt" => Metric::SuccessRateByPrompt,
            "cost_by_prompt" => Metric::CostByPrompt,
            _ => return None,
        })
    }
}

/// Pure state-machine transition. Returns `Some(next)` when `action` is enabled.
pub fn transition(state: State, action: &str) -> Option<State> {
    Some(match (state, action) {
        (State::Draft, "start") => State::Running,
        (State::Running, "conclude") => State::Concluded,
        (State::Concluded, "archive") => State::Archived,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions_follow_spec_order() {
        assert_eq!(transition(State::Draft, "start"), Some(State::Running));
        assert_eq!(
            transition(State::Running, "conclude"),
            Some(State::Concluded)
        );
        assert_eq!(
            transition(State::Concluded, "archive"),
            Some(State::Archived)
        );
    }

    #[test]
    fn archived_is_terminal() {
        assert_eq!(transition(State::Archived, "start"), None);
        assert_eq!(transition(State::Archived, "conclude"), None);
        assert_eq!(transition(State::Archived, "archive"), None);
    }

    #[test]
    fn no_backward_transitions() {
        assert_eq!(transition(State::Concluded, "start"), None);
        assert_eq!(transition(State::Running, "archive"), None);
    }

    #[test]
    fn metric_round_trip() {
        for m in [
            Metric::TokensPerSession,
            Metric::CostPerSession,
            Metric::SuccessRate,
            Metric::ToolLoops,
            Metric::DurationMinutes,
            Metric::FilesPerSession,
        ] {
            assert_eq!(Metric::parse(m.as_str()), Some(m));
        }
    }
}
