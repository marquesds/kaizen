// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared activity report for web and TUI.

mod activity;
mod build;
mod rollup;
mod types;

pub use build::{VisualizationQuery, build_report};
pub use types::*;
