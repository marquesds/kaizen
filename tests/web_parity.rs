// SPDX-License-Identifier: AGPL-3.0-or-later
//! Web tools must stay in lockstep with MCP tools.

include!("mcp_tool_names.inc");

#[test]
fn web_exposes_full_mcp_tool_set() {
    let mut expected = KAIZEN_MCP_TOOL_NAMES.to_vec();
    let mut got = kaizen::web::features::tool_names();
    expected.sort();
    got.sort();
    assert_eq!(got, expected);
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
fn web_features_have_product_metadata() {
    for feature in kaizen::web::features::all() {
        assert!(!feature.route.is_empty(), "{} route", feature.tool);
        assert!(!feature.section.is_empty(), "{} section", feature.tool);
        assert!(!feature.label.contains("kaizen_"), "{} label", feature.tool);
        assert!(!feature.renderer.is_empty(), "{} renderer", feature.tool);
        assert!(!feature.empty_state.is_empty(), "{} empty", feature.tool);
        assert!(!feature.error_state.is_empty(), "{} error", feature.tool);
    }
}

#[test]
fn mutating_workflows_are_marked() {
    for feature in kaizen::web::features::all() {
        let should_mutate = MUTATING_TOOLS.contains(&feature.tool);
        assert_eq!(feature.mutating, should_mutate, "{} mutating", feature.tool);
    }
}

const MUTATING_TOOLS: &[&str] = &[
    "kaizen_annotate_session",
    "kaizen_cases_archive",
    "kaizen_cases_create",
    "kaizen_cases_mine",
    "kaizen_exp_archive",
    "kaizen_exp_conclude",
    "kaizen_exp_new",
    "kaizen_exp_start",
    "kaizen_exp_tag",
    "kaizen_ingest_hook",
    "kaizen_init",
    "kaizen_metrics_index",
    "kaizen_review_dismiss",
    "kaizen_review_resolve",
    "kaizen_rules_create",
    "kaizen_rules_disable",
    "kaizen_rules_enable",
    "kaizen_rules_run",
    "kaizen_sync_run",
];
