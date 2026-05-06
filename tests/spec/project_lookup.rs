// SPDX-License-Identifier: AGPL-3.0-or-later
use itf::value::Value;
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct ProjectLookupState {
    match_count: i64,
    outcome: String,
}

#[derive(Debug, Default)]
struct ProjectLookupDriver {
    match_count: i64,
    outcome: String,
}

impl ProjectLookupDriver {
    fn apply_resolve(&mut self, n: i64) {
        self.match_count = n;
        self.outcome = if n == 1 {
            "Found".into()
        } else if n == 0 {
            "NotFound".into()
        } else {
            "Ambiguous".into()
        };
    }

    fn read_n(step: &Step) -> Result<i64> {
        let v = step
            .nondet_picks
            .get("n")
            .ok_or_else(|| anyhow::anyhow!("expected nondet pick `n` for resolve action"))?;
        match v {
            Value::Number(n) => Ok(*n),
            _ => anyhow::bail!("nondet `n` was not a number: {v:?}"),
        }
    }
}

impl State<ProjectLookupDriver> for ProjectLookupState {
    fn from_driver(d: &ProjectLookupDriver) -> Result<Self> {
        Ok(Self {
            match_count: d.match_count,
            outcome: d.outcome.clone(),
        })
    }
}

impl Driver for ProjectLookupDriver {
    type State = ProjectLookupState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => {
                self.match_count = 0;
                self.outcome = "NotFound".into();
            }
            "resolve" => {
                let n = Self::read_n(step)?;
                self.apply_resolve(n);
            }
            other => anyhow::bail!("unexpected action: {other}"),
        }
        Ok(())
    }
}

#[quint_run(spec = "specs/project-lookup.qnt", max_samples = 10, max_steps = 3)]
fn project_lookup_run() -> impl Driver {
    ProjectLookupDriver::default()
}
