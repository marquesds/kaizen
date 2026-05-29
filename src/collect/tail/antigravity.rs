// SPDX-License-Identifier: AGPL-3.0-or-later
//! Best-effort Antigravity adapter JSONL parser.

use crate::core::event::{Event, SessionRecord};
use anyhow::Result;
use std::path::Path;

pub fn scan_antigravity_session_file(path: &Path) -> Result<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_agent_session_file(path, "antigravity")
}

pub fn scan_antigravity_workspace(workspace: &Path) -> Vec<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_workspace(
        workspace,
        "ANTIGRAVITY_HOME",
        ".antigravity",
        ".antigravity",
        scan_antigravity_session_file,
    )
}

pub fn parse_antigravity_line(
    session_id: &str,
    seq: u64,
    base: u64,
    line: &str,
) -> Result<Option<Event>> {
    Ok(super::modern_jsonl::parse_common_line(
        "antigravity",
        session_id,
        seq,
        base,
        line,
    ))
}
