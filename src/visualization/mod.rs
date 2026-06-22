// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared read-only report for local visualization surfaces.

mod activity;
mod build;
mod status;
mod types;

pub(crate) use build::{BuiltReport, build_report_observed};
pub use build::{VisualizationLimits, VisualizationQuery, build_report};
pub(crate) use status::derive_status;
pub use types::*;
