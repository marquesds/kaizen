// SPDX-License-Identifier: AGPL-3.0-or-later
//! Web advertises only workflows implemented by its product surface.

use serde_json::json;

#[test]
fn web_advertises_only_session_observation() {
    let features = serde_json::to_value(kaizen::web::features::all()).unwrap();
    assert_eq!(features, json!([session_observation()]));
}

#[test]
fn web_executor_matches_feature_registry() {
    let mut expected = kaizen::web::features::tool_names();
    let mut got = kaizen::web::tools::WEB_TOOL_NAMES.to_vec();
    expected.sort();
    got.sort();
    assert_eq!(got, expected);
}

#[test]
fn web_workflows_are_observe_only() {
    assert!(
        kaizen::web::features::all()
            .into_iter()
            .all(|feature| !feature.mutating)
    );
}

fn session_observation() -> serde_json::Value {
    json!({
        "route": "/",
        "section": "Projects",
        "label": "Discover projects",
        "tool": "kaizen_sessions_list",
        "required_args": [],
        "mutating": false,
        "renderer": "select",
        "empty_state": "No observed projects yet.",
        "error_state": "Could not discover observed projects."
    })
}
