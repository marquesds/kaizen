// SPDX-License-Identifier: AGPL-3.0-or-later
pub mod antigravity;
pub mod claude;
pub mod claude_code;
pub mod codex;
pub mod codex_desktop;
mod codex_desktop_event;
pub mod copilot_cli;
pub mod copilot_vscode;
pub mod cursor;
pub mod cursor_state_db;
mod cursor_state_db_fields;
pub mod gemini;
pub mod goose;
pub mod kimi;
pub(crate) mod modern_jsonl;
pub(crate) mod modern_jsonl_event;
pub(crate) mod modern_jsonl_fields;
pub(crate) mod modern_jsonl_record;
pub mod openclaw;
pub mod opencode;
pub mod pi;
pub mod vibe;

use std::path::Path;

/// Earliest mtime (ms) of `.jsonl` files in `dir`. Returns 0 on failure.
pub fn dir_mtime_ms(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        })
        .min()
        .unwrap_or(0)
}

pub fn epoch_ms(t: u64) -> u64 {
    if t < 1_000_000_000_000 {
        t.saturating_mul(1000)
    } else {
        t
    }
}

pub fn value_ts_ms(v: &serde_json::Value) -> Option<u64> {
    v.as_u64()
        .map(epoch_ms)
        .or_else(|| v.as_str().and_then(rfc3339_ms))
}

fn rfc3339_ms(s: &str) -> Option<u64> {
    let dt = time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()?;
    u64::try_from(dt.unix_timestamp_nanos() / 1_000_000).ok()
}
