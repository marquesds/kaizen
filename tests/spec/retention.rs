// SPDX-License-Identifier: AGPL-3.0-or-later
use quint_connect::*;
use serde::Deserialize;

const HOT_DAYS: u32 = 30;
const WARM_DAYS: u32 = 90;

#[derive(Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecTier {
    Hot,
    Warm,
    Cold,
    Purged,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct RetentionState {
    tier: SpecTier,
    #[serde(with = "itf::de::As::<itf::de::Integer>")]
    age_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    Hot,
    Warm,
    Cold,
    Purged,
}

#[derive(Debug)]
struct RetentionDriver {
    tier: Tier,
    age_days: u32,
}

impl Default for RetentionDriver {
    fn default() -> Self {
        Self {
            tier: Tier::Hot,
            age_days: 0,
        }
    }
}

impl State<RetentionDriver> for RetentionState {
    fn from_driver(d: &RetentionDriver) -> Result<Self> {
        Ok(RetentionState {
            tier: match d.tier {
                Tier::Hot => SpecTier::Hot,
                Tier::Warm => SpecTier::Warm,
                Tier::Cold => SpecTier::Cold,
                Tier::Purged => SpecTier::Purged,
            },
            age_days: d.age_days,
        })
    }
}

impl Driver for RetentionDriver {
    type State = RetentionState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => {
                *self = RetentionDriver::default();
            },
            step => {
                *self = RetentionDriver::default();
            },
            tick_hot => {
                if !(self.tier == Tier::Hot && self.age_days < HOT_DAYS) {
                    return Err(anyhow::anyhow!("tick_hot not enabled"));
                }
                self.age_days += 1;
            },
            promote_warm => {
                if !(self.tier == Tier::Hot && self.age_days >= HOT_DAYS) {
                    return Err(anyhow::anyhow!("promote_warm not enabled"));
                }
                self.tier = Tier::Warm;
            },
            tick_warm => {
                if !(self.tier == Tier::Warm && self.age_days < WARM_DAYS) {
                    return Err(anyhow::anyhow!("tick_warm not enabled"));
                }
                self.age_days += 1;
            },
            promote_cold => {
                if !(self.tier == Tier::Warm && self.age_days >= WARM_DAYS) {
                    return Err(anyhow::anyhow!("promote_cold not enabled"));
                }
                self.tier = Tier::Cold;
            },
            purge => {
                if self.tier != Tier::Cold {
                    return Err(anyhow::anyhow!("purge not enabled"));
                }
                self.tier = Tier::Purged;
            },
            fast_forward_hot => {
                if self.tier != Tier::Hot {
                    return Err(anyhow::anyhow!("fast_forward_hot not enabled"));
                }
                self.age_days = HOT_DAYS;
            },
            fast_forward_warm => {
                if self.tier != Tier::Warm {
                    return Err(anyhow::anyhow!("fast_forward_warm not enabled"));
                }
                self.age_days = WARM_DAYS;
            }
        })
    }
}

#[quint_run(spec = "specs/retention.qnt", max_samples = 20, max_steps = 8)]
fn retention_run() -> impl Driver {
    RetentionDriver::default()
}
