// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

mod activity;
mod aggregates;
mod sessions;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TokenRead {
    pub(crate) input: u64,
    pub(crate) output: u64,
    pub(crate) reasoning: u64,
    pub(crate) cache_read: u64,
    pub(crate) cache_create: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ReportTotalsRead {
    pub(crate) session_count: u64,
    pub(crate) running_count: u64,
    pub(crate) event_count: u64,
    pub(crate) error_count: u64,
    pub(crate) tool_call_count: u64,
    pub(crate) cost_usd_e6: i64,
    pub(crate) tokens: TokenRead,
    pub(crate) token_event_count: u64,
    pub(crate) cost_event_count: u64,
    pub(crate) cost_session_count: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SessionSummaryRead {
    pub(crate) session: SessionRecord,
    pub(crate) last_event_ms: Option<u64>,
    pub(crate) event_count: u64,
    pub(crate) error_count: u64,
    pub(crate) tool_call_count: u64,
    pub(crate) cost_usd_e6: i64,
    pub(crate) tokens: TokenRead,
    pub(crate) top_tools: Vec<(String, u64)>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ActivityWindow {
    pub(crate) start_ms: u64,
    pub(crate) end_ms: u64,
    pub(crate) width_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActivityTotalRead {
    pub(crate) bin: usize,
    pub(crate) event_count: u64,
    pub(crate) session_count: u64,
    pub(crate) token_total: u64,
    pub(crate) cost_usd_e6: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct ActivityCountRead {
    pub(crate) bin: usize,
    pub(crate) name: String,
    pub(crate) count: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActivityRead {
    pub(crate) totals: Vec<ActivityTotalRead>,
    pub(crate) agents: Vec<ActivityCountRead>,
    pub(crate) kinds: Vec<ActivityCountRead>,
}

impl Store {
    pub fn files_for_session(&self, session_id: &str) -> Result<Vec<String>> {
        self.limited_files_for_session(session_id, usize::MAX)
    }

    pub(crate) fn limited_files_for_session(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT path FROM files_touched
             WHERE session_id = ?1 ORDER BY path ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, sql_limit(limit)], |row| row.get(0))?;
        rows.map(|row| row.map_err(Into::into)).collect()
    }
}

fn sql_limit(limit: usize) -> i64 {
    limit.min(i64::MAX as usize) as i64
}
