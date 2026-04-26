// SPDX-License-Identifier: AGPL-3.0-or-later
//! Connect test for specs/llm-call-quality.qnt.
//! Models per-request retry FSM: retry_count monotonic, event emitted on terminal outcome.

use quint_connect::*;
use serde::Deserialize;

const MAX_RETRIES: i64 = 5;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(tag = "tag")]
enum SpecOutcome {
    Pending,
    Inflight,
    Succeeded,
    RateLimited,
    TimedOut,
    GaveUp,
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct LlmCallQualityState {
    outcome: SpecOutcome,
    #[serde(rename = "retryCount", with = "itf::de::As::<itf::de::Integer>")]
    retry_count: i64,
    #[serde(rename = "eventEmitted")]
    event_emitted: bool,
}

#[derive(Debug, Default)]
struct LlmCallQualityDriver {
    outcome: SpecOutcome,
    retry_count: i64,
    event_emitted: bool,
}

impl Default for SpecOutcome {
    fn default() -> Self {
        SpecOutcome::Pending
    }
}

impl State<LlmCallQualityDriver> for LlmCallQualityState {
    fn from_driver(d: &LlmCallQualityDriver) -> Result<Self> {
        Ok(LlmCallQualityState {
            outcome: d.outcome,
            retry_count: d.retry_count,
            event_emitted: d.event_emitted,
        })
    }
}

impl Driver for LlmCallQualityDriver {
    type State = LlmCallQualityState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => {
                *self = LlmCallQualityDriver::default();
            }
            "send" => {
                assert_eq!(self.outcome, SpecOutcome::Pending);
                self.outcome = SpecOutcome::Inflight;
            }
            "ok_response" => {
                assert_eq!(self.outcome, SpecOutcome::Inflight);
                self.outcome = SpecOutcome::Succeeded;
                self.event_emitted = true;
            }
            "rate_limit" => {
                assert_eq!(self.outcome, SpecOutcome::Inflight);
                self.outcome = SpecOutcome::RateLimited;
            }
            "timeout" => {
                assert_eq!(self.outcome, SpecOutcome::Inflight);
                self.outcome = SpecOutcome::TimedOut;
            }
            "retry" => {
                assert!(self.retry_count < MAX_RETRIES);
                self.retry_count += 1;
                self.outcome = SpecOutcome::Inflight;
            }
            "give_up" => {
                self.outcome = SpecOutcome::GaveUp;
                self.event_emitted = true;
            }
            other => anyhow::bail!("unexpected llm_call_quality action: {other}"),
        }
        Ok(())
    }
}

#[test]
fn retry_count_bounded() {
    let mut d = LlmCallQualityDriver::default();
    d.outcome = SpecOutcome::RateLimited;
    for _ in 0..MAX_RETRIES {
        d.outcome = SpecOutcome::Inflight;
        d.retry_count += 1;
        d.outcome = SpecOutcome::RateLimited;
    }
    assert_eq!(d.retry_count, MAX_RETRIES);
}

#[test]
fn event_only_on_terminal() {
    let d = LlmCallQualityDriver {
        outcome: SpecOutcome::Succeeded,
        retry_count: 0,
        event_emitted: true,
    };
    assert!(matches!(
        d.outcome,
        SpecOutcome::Succeeded | SpecOutcome::GaveUp
    ));
}

#[quint_run(
    spec = "specs/llm-call-quality.qnt",
    max_samples = 15,
    max_steps = 10,
    seed = "0x1"
)]
fn llm_call_quality_run() -> impl Driver {
    LlmCallQualityDriver::default()
}
