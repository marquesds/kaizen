// SPDX-License-Identifier: AGPL-3.0-or-later

/// Max `ts_ms` still treated as transcript-only synthetic timing (seq-based fallbacks).
/// Rows below this use `sessions.started_at_ms` for time-window matching.
pub(crate) const SYNTHETIC_TS_CEILING_MS: i64 = 1_000_000_000_000;
pub(crate) const DEFAULT_CACHE_KIB: i64 = 8_192;
pub(crate) const DEFAULT_MMAP_MB: u64 = 32;

/// `sync_state` keys for agent rescan throttling and auto-prune.
pub const SYNC_STATE_LAST_AGENT_SCAN_MS: &str = "last_agent_scan_ms";
pub const SYNC_STATE_LAST_AUTO_PRUNE_MS: &str = "last_auto_prune_ms";
pub const SYNC_STATE_SEARCH_DIRTY_MS: &str = "search_dirty_ms";
