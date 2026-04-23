// SPDX-License-Identifier: AGPL-3.0-or-later
//! Heuristic retro engine (M5): pure ranking + IO at boundaries.

pub mod engine;
pub mod heuristics;
pub mod inputs;
pub mod scheduler;
pub mod types;

pub use engine::run;
pub use inputs::{load_inputs, prior_bet_fingerprints};
pub use types::{Bet, Inputs, Report};
