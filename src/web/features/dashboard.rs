// SPDX-License-Identifier: AGPL-3.0-or-later

use super::WebFeature;

pub(super) const FEATURES: &[WebFeature] = &[WebFeature {
    route: "/",
    section: "Projects",
    label: "Discover projects",
    tool: "kaizen_sessions_list",
    required_args: &[],
    mutating: false,
    renderer: "select",
    empty_state: "No observed projects yet.",
    error_state: "Could not discover observed projects.",
}];
