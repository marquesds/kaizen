// SPDX-License-Identifier: AGPL-3.0-or-later
//! Best-effort Kimi adapter JSONL parser.

use crate::core::event::{Event, SessionRecord};
use anyhow::Result;
use std::path::Path;

pub fn scan_kimi_session_file(path: &Path) -> Result<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_agent_session_file(path, "kimi")
}

pub fn scan_kimi_workspace(workspace: &Path) -> Vec<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_workspace(
        workspace,
        "KIMI_HOME",
        ".kimi",
        ".kimi",
        scan_kimi_session_file,
    )
}

pub fn parse_kimi_line(session_id: &str, seq: u64, base: u64, line: &str) -> Result<Option<Event>> {
    Ok(super::modern_jsonl::parse_common_line(
        "kimi", session_id, seq, base, line,
    ))
}
