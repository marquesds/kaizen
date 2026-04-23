use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct InitState {
    config_ready: bool,
    cursor_present: bool,
    cursor_patched: bool,
    cursor_backup: bool,
    claude_present: bool,
    claude_patched: bool,
    claude_backup: bool,
    skill_ready: bool,
}

#[derive(Debug, Default)]
struct InitDriver {
    config_ready: bool,
    cursor_present: bool,
    cursor_patched: bool,
    cursor_backup: bool,
    claude_present: bool,
    claude_patched: bool,
    claude_backup: bool,
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
    }

    fn seed_ready(&mut self) {
        self.seed_hook_files();
        self.config_ready = true;
        self.cursor_patched = true;
        self.cursor_backup = true;
        self.claude_patched = true;
        self.claude_backup = true;
        self.skill_ready = true;
    }

    fn run_init(&mut self) {
        self.config_ready = true;
        self.skill_ready = true;
        if self.cursor_present && !self.cursor_patched {
            self.cursor_backup = true;
            self.cursor_patched = true;
        }
        if self.claude_present && !self.claude_patched {
            self.claude_backup = true;
            self.claude_patched = true;
        }
    }
}

impl State<InitDriver> for InitState {
    fn from_driver(d: &InitDriver) -> Result<Self> {
        Ok(Self {
            config_ready: d.config_ready,
            cursor_present: d.cursor_present,
            cursor_patched: d.cursor_patched,
            cursor_backup: d.cursor_backup,
            claude_present: d.claude_present,
            claude_patched: d.claude_patched,
            claude_backup: d.claude_backup,
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
