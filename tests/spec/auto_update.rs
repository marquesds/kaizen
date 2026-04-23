// SPDX-License-Identifier: AGPL-3.0-or-later
//! Quint model: [specs/auto-update.qnt](../../specs/auto-update.qnt)
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct AutoState {
    op: i64,
    local: i64,
    remote: i64,
    auto_install: bool,
    approved: bool,
    integrity_ok: bool,
}

#[derive(Debug, Default)]
struct AutoDriver {
    op: i64,
    local: i64,
    remote: i64,
    auto_install: bool,
    approved: bool,
    integrity_ok: bool,
}

impl AutoDriver {
    fn init_d(&mut self) {
        self.op = 0;
        self.local = 1;
        self.remote = 0;
        self.auto_install = false;
        self.approved = false;
        self.integrity_ok = false;
    }

    fn enable_auto(&mut self) {
        self.auto_install = true;
    }

    fn user_approves_d(&mut self) {
        self.approved = true;
    }

    fn begin_check(&mut self) {
        assert_eq!(self.op, 0, "begin_check");
        self.op = 1;
    }

    fn check_no_update(&mut self) {
        assert_eq!(self.op, 1, "check_no_update");
        self.op = 0;
        self.remote = self.local;
    }

    fn check_update_available(&mut self) {
        assert_eq!(self.op, 1, "check_update_available");
        self.op = 0;
        self.remote = self.local + 1;
        self.integrity_ok = false;
    }

    fn check_fail(&mut self) {
        assert_eq!(self.op, 1, "check_fail");
        self.op = 0;
        self.remote = 0;
    }

    fn verify_ok(&mut self) {
        assert_eq!(self.op, 0, "verify_ok op");
        assert!(self.remote > 0, "verify_ok remote");
        assert!(self.remote > self.local, "verify_ok version");
        self.integrity_ok = true;
    }

    fn verify_fail(&mut self) {
        assert_eq!(self.op, 0, "verify_fail");
        assert!(self.remote > self.local, "verify_fail remote>local");
        self.integrity_ok = false;
    }

    fn begin_install(&mut self) {
        assert_eq!(self.op, 0, "begin_install op");
        assert!(self.remote > self.local, "begin_install");
        assert!(self.integrity_ok, "begin_install integrity");
        assert!(self.auto_install || self.approved, "begin_install policy");
        self.op = 2;
    }

    fn install_ok(&mut self) {
        assert_eq!(self.op, 2, "install_ok");
        self.op = 0;
        self.local = self.remote;
        self.approved = false;
        self.integrity_ok = false;
    }

    fn install_fail(&mut self) {
        assert_eq!(self.op, 2, "install_fail");
        self.op = 0;
        self.approved = false;
        self.integrity_ok = false;
    }
}

impl State<AutoDriver> for AutoState {
    fn from_driver(d: &AutoDriver) -> Result<Self> {
        Ok(Self {
            op: d.op,
            local: d.local,
            remote: d.remote,
            auto_install: d.auto_install,
            approved: d.approved,
            integrity_ok: d.integrity_ok,
        })
    }
}

impl Driver for AutoDriver {
    type State = AutoState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => self.init_d(),
            "enable_auto" => self.enable_auto(),
            "user_approves" => self.user_approves_d(),
            "begin_check" => self.begin_check(),
            "check_no_update" => self.check_no_update(),
            "check_update_available" => self.check_update_available(),
            "check_fail" => self.check_fail(),
            "verify_ok" => self.verify_ok(),
            "verify_fail" => self.verify_fail(),
            "begin_install" => self.begin_install(),
            "install_ok" => self.install_ok(),
            "install_fail" => self.install_fail(),
            other => anyhow::bail!("unexpected action: {other}"),
        }
        Ok(())
    }
}

#[quint_run(
    spec = "specs/auto-update.qnt",
    main = "auto_update",
    max_samples = 16,
    max_steps = 18,
    seed = "0x1"
)]
fn auto_update_run() -> impl Driver {
    AutoDriver::default()
}
