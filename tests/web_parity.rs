// SPDX-License-Identifier: AGPL-3.0-or-later
//! Web tools must stay in lockstep with MCP tools.

include!("mcp_tool_names.inc");

#[test]
fn web_exposes_full_mcp_tool_set() {
    let mut expected = KAIZEN_MCP_TOOL_NAMES.to_vec();
    let mut got = kaizen::web::tools::WEB_TOOL_NAMES.to_vec();
    expected.sort();
    got.sort();
    assert_eq!(got, expected);
}
