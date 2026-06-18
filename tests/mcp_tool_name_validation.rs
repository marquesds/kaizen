// SPDX-License-Identifier: AGPL-3.0-or-later
include!("mcp_tool_names.inc");

#[test]
fn all_mcp_tool_names_follow_protocol_format() {
    // @regression: rmcp warned on every daemon start for slash-delimited names.
    assert!(KAIZEN_MCP_TOOL_NAMES.iter().all(|name| {
        name.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    }));
}
