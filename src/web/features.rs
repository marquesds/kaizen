// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workflows implemented by the Web product surface.

mod dashboard;

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct WebFeature {
    pub route: &'static str,
    pub section: &'static str,
    pub label: &'static str,
    pub tool: &'static str,
    pub required_args: &'static [&'static str],
    pub mutating: bool,
    pub renderer: &'static str,
    pub empty_state: &'static str,
    pub error_state: &'static str,
}

pub fn all() -> Vec<WebFeature> {
    dashboard::FEATURES.to_vec()
}

pub fn tool_names() -> Vec<&'static str> {
    dashboard::FEATURES
        .iter()
        .map(|feature| feature.tool)
        .collect()
}
