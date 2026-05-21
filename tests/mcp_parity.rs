// SPDX-License-Identifier: AGPL-3.0-or-later
//! When you add an MCP `#[tool]`, update [mcp_tool_names.inc](mcp_tool_names.inc) and handler `src/mcp/handler.rs`.

include!("mcp_tool_names.inc");

#[test]
fn mcp_exposes_full_tool_set() {
    const EXPECTED: usize = 40;
    let names = KAIZEN_MCP_TOOL_NAMES;
    assert_eq!(
        names.len(),
        EXPECTED,
        "update mcp_tool_names::KAIZEN_MCP_TOOL_NAMES when adding/removing #[tool] in src/mcp/handler.rs (expected {EXPECTED})"
    );
    let mut v = names.to_vec();
    v.sort();
    for w in v.windows(2) {
        assert_ne!(w[0], w[1], "duplicate MCP tool name: {} / {}", w[0], w[1]);
    }
}
