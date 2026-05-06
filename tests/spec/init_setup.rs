// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct InitState {
    config_ready: bool,
    cursor_present: bool,
    cursor_patched: bool,
    cursor_backup: bool,
    legacy_cursor_local: bool,
    claude_present: bool,
    claude_patched: bool,
    claude_backup: bool,
    legacy_claude_local: bool,
    codex_present: bool,
    codex_patched: bool,
    codex_backup: bool,
    copilot_cli_present: bool,
    copilot_cli_patched: bool,
    copilot_cli_backup: bool,
    openclaw_present: bool,
    openclaw_patched: bool,
    openclaw_backup: bool,
    skill_ready: bool,
}

#[derive(Debug, Default)]
struct InitDriver {
    config_ready: bool,
    cursor_present: bool,
    cursor_patched: bool,
    cursor_backup: bool,
    legacy_cursor_local: bool,
    claude_present: bool,
    claude_patched: bool,
    claude_backup: bool,
    legacy_claude_local: bool,
    codex_present: bool,
    codex_patched: bool,
    codex_backup: bool,
    copilot_cli_present: bool,
    copilot_cli_patched: bool,
    copilot_cli_backup: bool,
    openclaw_present: bool,
    openclaw_patched: bool,
    openclaw_backup: bool,
    skill_ready: bool,
}

impl InitDriver {
    fn seed_empty(&mut self) {
        *self = Self::default();
    }

    fn seed_hook_files(&mut self) {
        self.seed_empty();
        self.cursor_present = true;
        self.claude_present = true;
        self.codex_present = true;
        self.copilot_cli_present = true;
        self.openclaw_present = true;
    }

    fn seed_ready(&mut self) {
        self.seed_hook_files();
        self.config_ready = true;
        self.cursor_patched = true;
        self.cursor_backup = true;
        self.claude_patched = true;
        self.claude_backup = true;
        self.codex_patched = true;
        self.codex_backup = true;
        self.copilot_cli_patched = true;
        self.copilot_cli_backup = true;
        self.openclaw_patched = true;
        self.openclaw_backup = true;
        self.skill_ready = true;
    }

    fn seed_legacy_local_wiring(&mut self) {
        self.seed_ready();
        self.legacy_cursor_local = true;
        self.legacy_claude_local = true;
    }

    fn run_init(&mut self) {
        self.config_ready = true;
        self.skill_ready = true;
        if self.cursor_present && !self.cursor_patched {
            self.cursor_backup = true;
        }
        if self.claude_present && !self.claude_patched {
            self.claude_backup = true;
        }
        if self.codex_present && !self.codex_patched {
            self.codex_backup = true;
        }
        if self.copilot_cli_present && !self.copilot_cli_patched {
            self.copilot_cli_backup = true;
        }
        if self.openclaw_present && !self.openclaw_patched {
            self.openclaw_backup = true;
        }
        self.cursor_present = true;
        self.cursor_patched = true;
        self.claude_present = true;
        self.claude_patched = true;
        self.codex_present = true;
        self.codex_patched = true;
        self.copilot_cli_present = true;
        self.copilot_cli_patched = true;
        self.openclaw_present = true;
        self.openclaw_patched = true;
        // legacy_cursor_local and legacy_claude_local are preserved (not cleared by init)
    }
}

impl State<InitDriver> for InitState {
    fn from_driver(d: &InitDriver) -> Result<Self> {
        Ok(Self {
            config_ready: d.config_ready,
            cursor_present: d.cursor_present,
            cursor_patched: d.cursor_patched,
            cursor_backup: d.cursor_backup,
            legacy_cursor_local: d.legacy_cursor_local,
            claude_present: d.claude_present,
            claude_patched: d.claude_patched,
            claude_backup: d.claude_backup,
            legacy_claude_local: d.legacy_claude_local,
            codex_present: d.codex_present,
            codex_patched: d.codex_patched,
            codex_backup: d.codex_backup,
            copilot_cli_present: d.copilot_cli_present,
            copilot_cli_patched: d.copilot_cli_patched,
            copilot_cli_backup: d.copilot_cli_backup,
            openclaw_present: d.openclaw_present,
            openclaw_patched: d.openclaw_patched,
            openclaw_backup: d.openclaw_backup,
            skill_ready: d.skill_ready,
        })
    }
}

impl Driver for InitDriver {
    type State = InitState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" | "seed_empty_workspace" => self.seed_empty(),
            "seed_existing_hook_files" => self.seed_hook_files(),
            "seed_ready_workspace" => self.seed_ready(),
            "seed_legacy_local_wiring" => self.seed_legacy_local_wiring(),
            "run_init" => self.run_init(),
            other => anyhow::bail!("unexpected action: {other}"),
        }
        Ok(())
    }
}

#[quint_run(spec = "specs/init-setup.qnt", max_samples = 10, max_steps = 5)]
fn init_setup_run() -> impl Driver {
    InitDriver::default()
}
