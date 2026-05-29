// SPDX-License-Identifier: AGPL-3.0-or-later
//! Best-effort Pi adapter JSONL parser.

use crate::core::event::{Event, SessionRecord};
use anyhow::Result;
use std::path::Path;

pub fn scan_pi_session_file(path: &Path) -> Result<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_agent_session_file(path, "pi")
}

pub fn scan_pi_workspace(workspace: &Path) -> Vec<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_workspace(workspace, "PI_HOME", ".pi", ".pi", scan_pi_session_file)
}

pub fn parse_pi_line(session_id: &str, seq: u64, base: u64, line: &str) -> Result<Option<Event>> {
    Ok(super::modern_jsonl::parse_common_line(
        "pi", session_id, seq, base, line,
    ))
}
