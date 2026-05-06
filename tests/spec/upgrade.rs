// SPDX-License-Identifier: AGPL-3.0-or-later
//! Quint model: [specs/upgrade.qnt](../../specs/upgrade.qnt)
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct UpgradeState {
    phase: i64,
    method: i64,
    last_ok: bool,
}

#[derive(Debug, Default)]
struct UpgradeDriver {
    phase: i64,
    method: i64,
    last_ok: bool,
}

impl UpgradeDriver {
    fn init_d(&mut self) {
        self.phase = 0;
        self.method = 0;
        self.last_ok = false;
    }

    fn begin_detect(&mut self) {
        assert_eq!(self.phase, 0, "begin_detect");
        self.phase = 1;
    }

    fn detect_homebrew(&mut self) {
        assert_eq!(self.phase, 1, "detect_homebrew");
        self.phase = 2;
        self.method = 1;
    }

    fn detect_cargo(&mut self) {
        assert_eq!(self.phase, 1, "detect_cargo");
        self.phase = 2;
        self.method = 2;
    }

    fn spawn_ok(&mut self) {
        assert_eq!(self.phase, 2, "spawn_ok phase");
        assert_ne!(self.method, 0, "spawn_ok method known");
        self.phase = 3;
        self.last_ok = true;
    }

    fn spawn_fail(&mut self) {
        assert_eq!(self.phase, 2, "spawn_fail phase");
        assert_ne!(self.method, 0, "spawn_fail method known");
        self.phase = 3;
        self.last_ok = false;
    }
}

impl State<UpgradeDriver> for UpgradeState {
    fn from_driver(d: &UpgradeDriver) -> Result<Self> {
        Ok(Self {
            phase: d.phase,
            method: d.method,
            last_ok: d.last_ok,
        })
    }
}

impl Driver for UpgradeDriver {
    type State = UpgradeState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => self.init_d(),
            "begin_detect" => self.begin_detect(),
            "detect_homebrew" => self.detect_homebrew(),
            "detect_cargo" => self.detect_cargo(),
            "spawn_ok" => self.spawn_ok(),
            "spawn_fail" => self.spawn_fail(),
            other => anyhow::bail!("unexpected action: {other}"),
        }
        Ok(())
    }
}

#[quint_run(
    spec = "specs/upgrade.qnt",
    main = "upgrade",
    max_samples = 16,
    max_steps = 8,
    seed = "0x2"
)]
fn upgrade_run() -> impl Driver {
    UpgradeDriver::default()
}
