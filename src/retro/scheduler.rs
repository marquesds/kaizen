//! Scheduling helpers for weekly retro.
//!
//! Recommended crontab (Sunday 09:00 local):
//! `0 9 * * 0 cd /path/to/repo && kaizen retro`

/// Human-readable default schedule documented for operators.
pub const DEFAULT_CRON: &str = "0 9 * * 0";
