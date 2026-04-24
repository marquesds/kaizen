// SPDX-License-Identifier: AGPL-3.0-or-later
//! Whether read-side commands use local SQLite, provider cache, or a merged view.

use clap::ValueEnum;

/// Data source for `retro` and observe-style commands.
#[derive(Clone, Copy, Debug, Default, ValueEnum, Eq, PartialEq)]
pub enum DataSource {
    /// Local SQLite and filesystem (default).
    #[default]
    Local,
    /// Rows from the `remote_*` cache (filled by `kaizen telemetry pull` when a provider is configured).
    Provider,
    /// Local rows plus `remote_*` with deduplication for overlapping keys.
    Mixed,
}
