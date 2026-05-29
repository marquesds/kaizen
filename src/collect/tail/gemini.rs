// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gemini JSONL adapter wrapper.

use crate::core::event::{Event, SessionRecord};
use anyhow::Result;
use std::path::Path;

pub fn scan_gemini_session_file(path: &Path) -> Result<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_agent_session_file(path, "gemini")
}

pub fn scan_gemini_workspace(workspace: &Path) -> Vec<(SessionRecord, Vec<Event>)> {
    super::modern_jsonl::scan_workspace(
        workspace,
        "GEMINI_HOME",
        ".gemini",
        ".gemini",
        scan_gemini_session_file,
    )
}

pub fn parse_gemini_line(sid: &str, seq: u64, base: u64, line: &str) -> Result<Option<Event>> {
    Ok(super::modern_jsonl::parse_common_line(
        "gemini", sid, seq, base, line,
    ))
}
