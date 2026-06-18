// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct DaemonHandshakeState {
    server_running: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    client_version: i32,
    subscribed: bool,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    queue_depth: i32,
    stopped: bool,
    background_returned: bool,
    status_result: String,
}

#[derive(Debug)]
struct DaemonHandshakeDriver {
    server_running: bool,
    client_version: i32,
    subscribed: bool,
    queue_depth: i32,
    stopped: bool,
    background_returned: bool,
    status_result: String,
}

impl Default for DaemonHandshakeDriver {
    fn default() -> Self {
        Self {
            server_running: false,
            client_version: 1,
            subscribed: false,
            queue_depth: 0,
            stopped: false,
            background_returned: false,
            status_result: "none".into(),
        }
    }
}

impl State<DaemonHandshakeDriver> for DaemonHandshakeState {
    fn from_driver(d: &DaemonHandshakeDriver) -> Result<Self> {
        Ok(Self {
            server_running: d.server_running,
            client_version: d.client_version,
            subscribed: d.subscribed,
            queue_depth: d.queue_depth,
            stopped: d.stopped,
            background_returned: d.background_returned,
            status_result: d.status_result.clone(),
        })
    }
}

impl Driver for DaemonHandshakeDriver {
    type State = DaemonHandshakeState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => *self = Self::default(),
            step => *self = Self::default(),
            start => {
                self.server_running = true;
                self.stopped = false;
            },
            start_background => {
                self.server_running = true;
                self.stopped = false;
                self.background_returned = true;
            },
            bad_version => {
                if !self.server_running {
                    anyhow::bail!("bad_version not enabled");
                }
                self.client_version = 0;
            },
            retry_supported => {
                if !self.server_running || self.client_version == 1 {
                    anyhow::bail!("retry_supported not enabled");
                }
                self.client_version = 1;
            },
            subscribe => {
                if !self.server_running || self.client_version != 1 {
                    anyhow::bail!("subscribe not enabled");
                }
                self.subscribed = true;
            },
            unsubscribe => {
                if !self.subscribed {
                    anyhow::bail!("unsubscribe not enabled");
                }
                self.subscribed = false;
            },
            stop => {
                if !self.server_running || self.queue_depth != 0 {
                    anyhow::bail!("stop not enabled");
                }
                self.server_running = false;
                self.subscribed = false;
                self.stopped = true;
                self.queue_depth = 0;
                self.background_returned = false;
            },
            status_running => {
                if !self.server_running || self.client_version != 1 {
                    anyhow::bail!("status_running not enabled");
                }
                self.status_result = "running".into();
            },
            status_stopped => {
                if self.server_running {
                    anyhow::bail!("status_stopped not enabled");
                }
                self.status_result = "stopped".into();
            },
            status_protocol_error => {
                if !self.server_running || self.client_version == 1 {
                    anyhow::bail!("status_protocol_error not enabled");
                }
                self.status_result = "error".into();
            },
        })
    }
}

pub(super) fn driver() -> impl Driver {
    DaemonHandshakeDriver::default()
}
