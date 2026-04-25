// SPDX-License-Identifier: AGPL-3.0-or-later
//! Experiment binding + stats v0. See `docs/experiments.md`.

pub mod binding;
pub mod engine;
pub mod metric;
pub mod stats;
pub mod store;
pub mod types;

pub use engine::{Report, run, to_markdown};
pub use types::{
    Binding, Classification, Criterion, Direction, Experiment, GuardrailResult, GuardrailSpec,
    Metric, State, transition,
};
