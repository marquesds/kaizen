// SPDX-License-Identifier: AGPL-3.0-or-later
// Included by `mcp_parity.rs` and `mcp_tools_smoke.rs` — must match `#[tool(name = ...)]` in src/mcp/handler.rs.

/// Sorted. Must match `list_all_tools` / `list_tools` from `KaizenMcp`.
pub const KAIZEN_MCP_TOOL_NAMES: &[&str] = &[
    "get_session_span_tree",
    "kaizen_annotate_session",
    "kaizen_capabilities",
    "kaizen_exp_archive",
    "kaizen_exp_conclude",
    "kaizen_exp_list",
    "kaizen_exp_new",
    "kaizen_exp_report",
    "kaizen_exp_start",
    "kaizen_exp_status",
    "kaizen_exp_tag",
    "kaizen_ingest_hook",
    "kaizen_init",
    "kaizen_insights",
    "kaizen_metrics",
    "kaizen_metrics_index",
    "kaizen_retro",
    "kaizen_session_show",
    "kaizen_sessions_list",
    "kaizen_summary",
    "kaizen_sync_run",
    "kaizen_sync_status",
    "kaizen_tui",
];
