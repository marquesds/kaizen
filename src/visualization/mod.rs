// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared read-only report for local visualization surfaces.

mod activity;
mod build;
mod types;

pub(crate) use build::{BuiltReport, build_report_observed, derive_status};
pub use build::{VisualizationLimits, VisualizationQuery, build_report};
pub use types::*;
