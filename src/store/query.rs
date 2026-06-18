// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytics query facade over canonical SQLite data.

use crate::store::sqlite::{Store, SummaryStats};
use anyhow::Result;
use std::path::Path;

pub struct QueryStore;

impl QueryStore {
    pub fn open(_root: &Path) -> Result<Self> {
        Ok(Self)
    }

    pub fn summary_stats(&self, sqlite: &Store, workspace: &str) -> Result<SummaryStats> {
        sqlite.summary_stats(workspace)
    }

    pub fn cold_event_count(&self) -> Result<u64> {
        Ok(0)
    }
}
