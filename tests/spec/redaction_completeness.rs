// SPDX-License-Identifier: AGPL-3.0-or-later
use itf::value::Value;
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct RedactionState {
    staged_forbidden: bool,
    redacted_forbidden: bool,
    saw_forbidden_input: bool,
    allowlist_team: bool,
    staged_cleartext_team: bool,
    out_cleartext_team: bool,
}

#[derive(Default)]
struct RedactionDriver {
    staged_forbidden: bool,
    redacted_forbidden: bool,
    saw_forbidden_input: bool,
    allowlist_team: bool,
    staged_cleartext_team: bool,
    out_cleartext_team: bool,
}

impl State<RedactionDriver> for RedactionState {
    fn from_driver(d: &RedactionDriver) -> Result<Self> {
        Ok(RedactionState {
            staged_forbidden: d.staged_forbidden,
            redacted_forbidden: d.redacted_forbidden,
            saw_forbidden_input: d.saw_forbidden_input,
            allowlist_team: d.allowlist_team,
            staged_cleartext_team: d.staged_cleartext_team,
            out_cleartext_team: d.out_cleartext_team,
        })
    }
}

impl RedactionDriver {
    fn read_bool_pick(step: &Step, key: &str) -> Result<bool> {
        let v = step
            .nondet_picks
            .get(key)
            .ok_or_else(|| anyhow::anyhow!("nondet pick `{key}`"))?;
        match v {
            Value::Bool(b) => Ok(*b),
            _ => anyhow::bail!("nondet `{key}` not bool: {v:?}"),
        }
    }
}

impl Driver for RedactionDriver {
    type State = RedactionState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.staged_forbidden = false;
                self.redacted_forbidden = false;
                self.saw_forbidden_input = false;
                self.allowlist_team = false;
                self.staged_cleartext_team = false;
                self.out_cleartext_team = false;
            },
            step => {}
            set_allowlist_team => {
                self.allowlist_team = Self::read_bool_pick(step, "on")?;
            },
            enable_team_allowlist => {
                self.allowlist_team = true;
            },
            stage(bad: bool) => {
                self.staged_forbidden = bad;
            },
            stage_team_label => {
                self.staged_forbidden = false;
                self.staged_cleartext_team = true;
            },
            redact => {
                self.redacted_forbidden = if self.staged_forbidden {
                    false
                } else {
                    self.staged_forbidden
                };
                self.saw_forbidden_input = if self.staged_forbidden {
                    true
                } else {
                    self.saw_forbidden_input
                };
                self.out_cleartext_team = !self.staged_forbidden
                    && self.allowlist_team
                    && self.staged_cleartext_team;
                self.staged_forbidden = false;
                self.staged_cleartext_team = false;
            }
        })
    }
}

#[quint_run(
    spec = "specs/redaction-completeness.qnt",
    max_samples = 16,
    max_steps = 10
)]
fn redaction_completeness_run() -> impl Driver {
    RedactionDriver::default()
}
