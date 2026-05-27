// SPDX-License-Identifier: AGPL-3.0-or-later
//! Web feature registry. Every MCP tool maps to a visible product workflow.

mod analysis;
mod dashboard;
mod experiments;
mod session;
mod settings;

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
    sections().into_iter().flatten().copied().collect()
}

pub fn tool_names() -> Vec<&'static str> {
    all().into_iter().map(|feature| feature.tool).collect()
}

fn sections() -> [&'static [WebFeature]; 5] {
    [
        dashboard::FEATURES,
        session::FEATURES,
        analysis::FEATURES,
        experiments::FEATURES,
        settings::FEATURES,
    ]
}

pub(super) const fn wf(
    route: &'static str,
    section: &'static str,
    label: &'static str,
    tool: &'static str,
    required_args: &'static [&'static str],
    mutating: bool,
    renderer: &'static str,
) -> WebFeature {
    WebFeature {
        route,
        section,
        label,
        tool,
        required_args,
        mutating,
        renderer,
        empty_state: "No records yet.",
        error_state: "Action failed. Open Developer details for the raw response.",
    }
}
