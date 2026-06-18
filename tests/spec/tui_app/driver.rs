// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;
#[derive(Debug, Eq, PartialEq, Deserialize)]
pub(super) struct TuiState {
    phase: String,
    dirty: bool,
    #[serde(rename = "reportDirty")]
    report_dirty: bool,
    #[serde(rename = "reportComputing")]
    report_computing: bool,
    #[serde(rename = "reportReady")]
    report_ready: bool,
    #[serde(rename = "walEvents")]
    wal_events: i64,
    #[serde(rename = "manualRefreshes")]
    manual_refreshes: i64,
    refreshes: i64,
    #[serde(rename = "reportPublishes")]
    report_publishes: i64,
    #[serde(rename = "totalSessions")]
    total_sessions: i64,
    cursor: i64,
    #[serde(rename = "fetchedStart")]
    fetched_start: i64,
    #[serde(rename = "fetchedEnd")]
    fetched_end: i64,
    #[serde(rename = "pageLoading")]
    page_loading: bool,
    #[serde(rename = "loadRequests")]
    load_requests: i64,
}
#[derive(Debug)]
pub(super) struct TuiDriver {
    phase: String,
    dirty: bool,
    report_dirty: bool,
    report_computing: bool,
    report_ready: bool,
    wal_events: i64,
    manual_refreshes: i64,
    refreshes: i64,
    report_publishes: i64,
    total_sessions: i64,
    cursor: i64,
    fetched_start: i64,
    fetched_end: i64,
    page_loading: bool,
    load_requests: i64,
}
impl Default for TuiDriver {
    fn default() -> Self {
        Self {
            phase: "Boot".into(),
            dirty: false,
            report_dirty: true,
            report_computing: false,
            report_ready: false,
            wal_events: 0,
            manual_refreshes: 0,
            refreshes: 0,
            report_publishes: 0,
            total_sessions: 0,
            cursor: 0,
            fetched_start: 0,
            fetched_end: 0,
            page_loading: false,
            load_requests: 0,
        }
    }
}
impl State<TuiDriver> for TuiState {
    fn from_driver(driver: &TuiDriver) -> Result<Self> {
        Ok(TuiState {
            phase: driver.phase.clone(),
            dirty: driver.dirty,
            report_dirty: driver.report_dirty,
            report_computing: driver.report_computing,
            report_ready: driver.report_ready,
            wal_events: driver.wal_events,
            manual_refreshes: driver.manual_refreshes,
            refreshes: driver.refreshes,
            report_publishes: driver.report_publishes,
            total_sessions: driver.total_sessions,
            cursor: driver.cursor,
            fetched_start: driver.fetched_start,
            fetched_end: driver.fetched_end,
            page_loading: driver.page_loading,
            load_requests: driver.load_requests,
        })
    }
}

impl Driver for TuiDriver {
    type State = TuiState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                *self = Self::default();
            },
            step => {
                *self = Self::default();
            },
            become_ready => {
                require_phase(&self.phase, "Boot", "become_ready")?;
                self.phase = "Interactive".into();
            },
            wal_change => {
                require_phase(&self.phase, "Interactive", "wal_change")?;
                self.dirty = true;
                self.wal_events += 1;
            },
            coalesced_refresh => {
                require_phase(&self.phase, "Interactive", "coalesced_refresh")?;
                if !self.dirty {
                    anyhow::bail!("coalesced_refresh not enabled");
                }
                self.dirty = false;
                self.report_dirty = true;
                self.refreshes += 1;
            },
            manual_refresh => {
                require_phase(&self.phase, "Interactive", "manual_refresh")?;
                self.dirty = false;
                self.report_dirty = true;
                self.manual_refreshes += 1;
                self.refreshes += 1;
            },
            report_recompute => {
                require_phase(&self.phase, "Interactive", "report_recompute")?;
                if !self.report_dirty || self.report_computing {
                    anyhow::bail!("report_recompute not enabled");
                }
                self.report_dirty = false;
                self.report_computing = true;
            },
            report_publish => {
                require_phase(&self.phase, "Interactive", "report_publish")?;
                if !self.report_computing {
                    anyhow::bail!("report_publish not enabled");
                }
                self.report_computing = false;
                self.report_ready = true;
                self.report_publishes += 1;
            },
            page_load_request => {
                require_phase(&self.phase, "Interactive", "page_load_request")?;
                if self.page_loading {
                    anyhow::bail!("page_load_request double-load");
                }
                self.page_loading = true;
                self.load_requests += 1;
            },
            page_load_complete => {
                require_phase(&self.phase, "Interactive", "page_load_complete")?;
                if !self.page_loading {
                    anyhow::bail!("page_load_complete not enabled");
                }
                self.total_sessions = 20;
                self.fetched_start = 0;
                self.fetched_end = 20;
                self.page_loading = false;
            },
            scroll_down => {
                require_phase(&self.phase, "Interactive", "scroll_down")?;
                if self.total_sessions <= 0
                    || self.cursor + 1 >= self.total_sessions
                    || self.cursor + 1 >= self.fetched_end
                {
                    anyhow::bail!("scroll_down not enabled");
                }
                self.cursor += 1;
            },
            filter_reset => {
                require_phase(&self.phase, "Interactive", "filter_reset")?;
                self.total_sessions = 0;
                self.cursor = 0;
                self.fetched_start = 0;
                self.fetched_end = 0;
                self.page_loading = false;
            },
            request_quit => {
                require_phase(&self.phase, "Interactive", "request_quit")?;
                self.phase = "Draining".into();
            },
            finish_shutdown => {
                require_phase(&self.phase, "Draining", "finish_shutdown")?;
                self.phase = "Exited".into();
            },
        })
    }
}

fn require_phase(actual: &str, expected: &str, action: &str) -> Result<()> {
    if actual != expected {
        anyhow::bail!("{action} not enabled");
    }
    Ok(())
}
