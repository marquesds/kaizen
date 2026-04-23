// SPDX-License-Identifier: AGPL-3.0-or-later
//! When you add a top-level `kaizen` CLI command, add a `kaizen_*` MCP tool and update this list.

const KAIZEN_MCP_TOOL_NAMES: &[&str] = &[
    "kaizen_ingest_hook",
    "kaizen_sessions_list",
    "kaizen_session_show",
    "kaizen_summary",
    "kaizen_tui",
    "kaizen_init",
    "kaizen_insights",
    "kaizen_metrics",
    "kaizen_metrics_index",
    "kaizen_sync_run",
    "kaizen_sync_status",
    "kaizen_exp_new",
    "kaizen_exp_list",
    "kaizen_exp_status",
    "kaizen_exp_tag",
    "kaizen_exp_report",
    "kaizen_exp_conclude",
    "kaizen_retro",
];

#[test]
fn mcp_exposes_full_tool_set() {
    assert_eq!(
        KAIZEN_MCP_TOOL_NAMES.len(),
        18,
        "update KAIZEN_MCP_TOOL_NAMES when adding tools"
    );
    let mut v = KAIZEN_MCP_TOOL_NAMES.to_vec();
    v.sort();
    for w in v.windows(2) {
        assert_ne!(w[0], w[1], "duplicate MCP tool name: {} / {}", w[0], w[1]);
    }
}
