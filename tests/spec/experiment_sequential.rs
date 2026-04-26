// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connect test for specs/experiment-sequential.qnt.
//! Uses an inline `decide` that mirrors what `stats::sequential` will implement.
use quint_connect::*;
use serde::Deserialize;

const MIN_N: i64 = 30;

// --- Quint Decision type ---

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecDecision {
    Insufficient,
    Inconclusive,
    Significant,
}

// --- State ---

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct SeqState {
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    n_control: i64,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    n_treatment: i64,
    ever_significant: bool,
    decision: SpecDecision,
}

// --- Driver ---

#[derive(Debug, Default)]
struct SeqDriver {
    n_control: i64,
    n_treatment: i64,
    ever_significant: bool,
}

fn decide(nc: i64, nt: i64, ever_sig: bool) -> SpecDecision {
    if ever_sig {
        SpecDecision::Significant
    } else if nc < MIN_N || nt < MIN_N {
        SpecDecision::Insufficient
    } else {
        SpecDecision::Inconclusive
    }
}

impl State<SeqDriver> for SeqState {
    fn from_driver(d: &SeqDriver) -> Result<Self> {
        Ok(SeqState {
            n_control: d.n_control,
            n_treatment: d.n_treatment,
            ever_significant: d.ever_significant,
            decision: decide(d.n_control, d.n_treatment, d.ever_significant),
        })
    }
}

impl Driver for SeqDriver {
    type State = SeqState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                self.n_control        = 0;
                self.n_treatment      = 0;
                self.ever_significant = false;
            },
            step => {
                self.n_control        = 0;
                self.n_treatment      = 0;
                self.ever_significant = false;
            },
            observe_control(crosses: bool) => {
                self.n_control += 1;
                self.ever_significant = self.ever_significant
                    || (crosses && self.n_control >= MIN_N && self.n_treatment >= MIN_N);
            },
            observe_treatment(crosses: bool) => {
                self.n_treatment += 1;
                self.ever_significant = self.ever_significant
                    || (crosses && self.n_control >= MIN_N && self.n_treatment >= MIN_N);
            }
        })
    }
}

// Spot-check: once significant, stays significant.
#[test]
fn significant_is_sticky() {
    let d = SeqDriver {
        n_control: MIN_N,
        n_treatment: MIN_N,
        ever_significant: true,
    };
    assert_eq!(
        decide(d.n_control, d.n_treatment, d.ever_significant),
        SpecDecision::Significant
    );
    // More observations with no new evidence: still Significant.
    assert_eq!(
        decide(d.n_control + 100, d.n_treatment + 100, true),
        SpecDecision::Significant
    );
}

// Spot-check: insufficient when sample too small.
#[test]
fn insufficient_when_small() {
    assert_eq!(decide(5, 5, false), SpecDecision::Insufficient);
}

#[quint_run(
    spec = "specs/experiment-sequential.qnt",
    max_samples = 20,
    max_steps = 8,
    seed = "0x3"
)]
fn experiment_sequential_run() -> impl Driver {
    SeqDriver::default()
}
