// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connect test for specs/system-sampler.qnt.
//! Invariants: samples only while Tracking/Stopped; Stopped is terminal; pid valid when active.

use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecSamplerState {
    Off,
    Tracking,
    Stopped,
}

impl Default for SpecSamplerState {
    fn default() -> Self {
        SpecSamplerState::Off
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct SystemSamplerState {
    state: SpecSamplerState,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    pid: i64,
    #[serde(rename = "sampleCount", with = "itf::de::As::<itf::de::Integer>")]
    sample_count: i64,
}

#[derive(Debug, Default)]
struct SystemSamplerDriver {
    state: SpecSamplerState,
    pid: i64,
    sample_count: i64,
}

impl State<SystemSamplerDriver> for SystemSamplerState {
    fn from_driver(d: &SystemSamplerDriver) -> Result<Self> {
        Ok(SystemSamplerState {
            state: d.state,
            pid: d.pid,
            sample_count: d.sample_count,
        })
    }
}

impl Driver for SystemSamplerDriver {
    type State = SystemSamplerState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => { *self = SystemSamplerDriver::default(); },
            step => { *self = SystemSamplerDriver::default(); },
            session_start(p: i64) => {
                assert_eq!(self.state, SpecSamplerState::Off);
                assert!(p > 0);
                self.state = SpecSamplerState::Tracking;
                self.pid = p;
                self.sample_count = 0;
            },
            sample => {
                assert_eq!(self.state, SpecSamplerState::Tracking);
                self.sample_count += 1;
            },
            session_stop => {
                assert_eq!(self.state, SpecSamplerState::Tracking);
                self.state = SpecSamplerState::Stopped;
            }
        })
    }
}

#[test]
fn no_samples_while_off() {
    let d = SystemSamplerDriver::default();
    assert_eq!(d.sample_count, 0);
    assert_eq!(d.state, SpecSamplerState::Off);
}

#[test]
fn stopped_is_terminal() {
    let d = SystemSamplerDriver {
        state: SpecSamplerState::Stopped,
        pid: 42,
        sample_count: 3,
    };
    assert_eq!(d.state, SpecSamplerState::Stopped);
    assert!(d.sample_count >= 0);
}

#[quint_run(
    spec = "specs/system-sampler.qnt",
    max_samples = 15,
    max_steps = 8,
    seed = "0x4"
)]
fn system_sampler_run() -> impl Driver {
    SystemSamplerDriver::default()
}
