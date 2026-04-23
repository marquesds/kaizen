// SPDX-License-Identifier: AGPL-3.0-or-later
pub mod claude;
pub mod codex;
pub mod copilot_cli;
pub mod copilot_vscode;
pub mod cursor;
pub mod goose;
pub mod opencode;

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
