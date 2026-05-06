// SPDX-License-Identifier: AGPL-3.0-or-later
use itf::value::Value;
use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum Outcome {
    Found,
    #[default]
    NotFound,
    Ambiguous,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct ProjectLookupState {
    #[serde(rename = "match_count", with = "itf::de::As::<itf::de::Integer>")]
    match_count: i64,
    outcome: Outcome,
}

#[derive(Debug, Default)]
struct ProjectLookupDriver {
    match_count: i64,
    outcome: Outcome,
}

impl ProjectLookupDriver {
    fn apply_resolve(&mut self, n: i64) {
        self.match_count = n;
        self.outcome = match n {
            1 => Outcome::Found,
            0 => Outcome::NotFound,
            _ => Outcome::Ambiguous,
        };
    }

    fn read_n(step: &Step) -> Result<i64> {
        let v = step
            .nondet_picks
            .get("n")
            .ok_or_else(|| anyhow::anyhow!("expected nondet pick `n` for resolve action"))?;
        match v {
            Value::Number(n) => Ok(*n),
            Value::BigInt(n) => {
                i64::try_from(n.clone().into_inner())
                    .map_err(|_| anyhow::anyhow!("nondet `n` overflows i64: {v:?}"))
            }
            _ => anyhow::bail!("nondet `n` was not a number: {v:?}"),
        }
    }
}

impl State<ProjectLookupDriver> for ProjectLookupState {
    fn from_driver(d: &ProjectLookupDriver) -> Result<Self> {
        Ok(Self {
            match_count: d.match_count,
            outcome: d.outcome,
        })
    }
}

impl Driver for ProjectLookupDriver {
    type State = ProjectLookupState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => *self = ProjectLookupDriver::default(),
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
