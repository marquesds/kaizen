// SPDX-License-Identifier: AGPL-3.0-or-later
//! Quint model-check of MCP edge invariants (TUI stub, sync run once vs daemon), see
//! [specs/mcp-server.qnt](specs/mcp-server.qnt).

use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct McpState {
    connected: bool,
    #[serde(rename = "tuiCalls")]
    tui_calls: i64,
    #[serde(rename = "tuiUnavailable")]
    tui_unavailable: i64,
    #[serde(rename = "syncDaemonAttempts")]
    sync_daemon_attempts: i64,
    #[serde(rename = "syncDaemonRejects")]
    sync_daemon_rejects: i64,
    #[serde(rename = "syncOnceFlushes")]
    sync_once_flushes: i64,
}

#[derive(Debug, Default)]
struct McpDriver {
    connected: bool,
    tui_calls: i64,
    tui_unavailable: i64,
    sync_daemon_attempts: i64,
    sync_daemon_rejects: i64,
    sync_once_flushes: i64,
}

impl State<McpDriver> for McpState {
    fn from_driver(d: &McpDriver) -> Result<Self> {
        Ok(McpState {
            connected: d.connected,
            tui_calls: d.tui_calls,
            tui_unavailable: d.tui_unavailable,
            sync_daemon_attempts: d.sync_daemon_attempts,
            sync_daemon_rejects: d.sync_daemon_rejects,
            sync_once_flushes: d.sync_once_flushes,
        })
    }
}

impl Driver for McpDriver {
    type State = McpState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => {
                *self = McpDriver::default();
            }
            "client_connect" => {
                if !self.connected {
                    self.connected = true;
                }
            }
            "tui_call" => {
                if self.connected {
                    self.tui_calls += 1;
                    self.tui_unavailable += 1;
                }
            }
            "sync_run_daemon" => {
                if self.connected {
                    self.sync_daemon_attempts += 1;
                    self.sync_daemon_rejects += 1;
                }
            }
            "sync_run_once" => {
                if self.connected {
                    self.sync_once_flushes += 1;
                }
            }
            other => anyhow::bail!("unexpected mcp action: {other}"),
        }
        Ok(())
    }
}

#[quint_run(spec = "specs/mcp-server.qnt", max_samples = 10, max_steps = 8)]
fn mcp_server_run() -> impl Driver {
    McpDriver::default()
}
