use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct RedactionState {
    staged_forbidden: bool,
    redacted_forbidden: bool,
    saw_forbidden_input: bool,
}

#[derive(Default)]
struct RedactionDriver {
    staged_forbidden: bool,
    redacted_forbidden: bool,
    saw_forbidden_input: bool,
}

impl State<RedactionDriver> for RedactionState {
    fn from_driver(d: &RedactionDriver) -> Result<Self> {
        Ok(RedactionState {
            staged_forbidden: d.staged_forbidden,
            redacted_forbidden: d.redacted_forbidden,
            saw_forbidden_input: d.saw_forbidden_input,
        })
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
            },
            step => {}
            stage(bad: bool) => {
                self.staged_forbidden = bad;
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
                self.staged_forbidden = false;
            }
        })
    }
}

#[quint_run(spec = "specs/redaction-completeness.qnt", max_samples = 12, max_steps = 8)]
fn redaction_completeness_run() -> impl Driver {
    RedactionDriver::default()
}
