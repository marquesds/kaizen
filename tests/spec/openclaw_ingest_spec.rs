// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum FilterState {
    Scanning,
    Accepted,
    Rejected,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct OpenclawIngestState {
    ws_filter: FilterState,
    lines_read: i64,
    session_produced: bool,
}

#[derive(Debug, Default)]
struct OpenclawIngestDriver {
    ws_filter: String,
    lines_read: i64,
    session_produced: bool,
}

impl OpenclawIngestDriver {
    fn reset(&mut self) {
        self.ws_filter = "Scanning".into();
        self.lines_read = 0;
        self.session_produced = false;
    }
}

impl State<OpenclawIngestDriver> for OpenclawIngestState {
    fn from_driver(d: &OpenclawIngestDriver) -> Result<Self> {
        let ws_filter = match d.ws_filter.as_str() {
            "Accepted" => FilterState::Accepted,
            "Rejected" => FilterState::Rejected,
            _ => FilterState::Scanning,
        };
        Ok(OpenclawIngestState {
            ws_filter,
            lines_read: d.lines_read,
            session_produced: d.session_produced,
        })
    }
}

impl Driver for OpenclawIngestDriver {
    type State = OpenclawIngestState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => self.reset(),
            "read_matching_cwd" => {
                if self.ws_filter != "Scanning" {
                    anyhow::bail!("read_matching_cwd not enabled");
                }
                self.ws_filter = "Accepted".into();
                self.lines_read += 1;
            }
            "read_no_cwd" => {
                if self.ws_filter != "Scanning" {
                    anyhow::bail!("read_no_cwd not enabled");
                }
                self.lines_read += 1;
            }
            "read_wrong_cwd" => {
                if self.ws_filter != "Scanning" {
                    anyhow::bail!("read_wrong_cwd not enabled");
                }
                self.ws_filter = "Rejected".into();
                self.lines_read += 1;
            }
            "produce_session" => {
                if self.ws_filter != "Accepted" {
                    anyhow::bail!("produce_session not enabled");
                }
                self.session_produced = true;
            }
            "skip_session" => {
                if self.ws_filter == "Accepted" {
                    anyhow::bail!("skip_session not enabled when Accepted");
                }
                self.session_produced = false;
            }
            other => anyhow::bail!("unexpected action: {other}"),
        }
        Ok(())
    }
}

#[quint_run(
    spec = "specs/openclaw-ingest.qnt",
    max_samples = 15,
    max_steps = 6,
    seed = "0x3"
)]
fn openclaw_ingest_run() -> impl Driver {
    OpenclawIngestDriver {
        ws_filter: "Scanning".into(),
        lines_read: 0,
        session_produced: false,
    }
}
