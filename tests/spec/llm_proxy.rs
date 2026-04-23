// SPDX-License-Identifier: AGPL-3.0-or-later
//! Quint model of LLM HTTP proxy invariants, see [specs/llm-proxy.qnt](specs/llm-proxy.qnt).

use quint_connect::*;
use serde::Deserialize;

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct LlmProxyState {
    listening: bool,
    #[serde(rename = "eventSeq")]
    event_seq: i64,
    #[serde(rename = "successForwards")]
    success_forwards: i64,
    #[serde(rename = "errorForwards")]
    error_forwards: i64,
    #[serde(rename = "unboundRejects")]
    unbound_rejects: i64,
}

#[derive(Debug, Default)]
struct LlmProxyDriver {
    listening: bool,
    event_seq: i64,
    success_forwards: i64,
    error_forwards: i64,
    unbound_rejects: i64,
}

impl State<LlmProxyDriver> for LlmProxyState {
    fn from_driver(d: &LlmProxyDriver) -> Result<Self> {
        Ok(LlmProxyState {
            listening: d.listening,
            event_seq: d.event_seq,
            success_forwards: d.success_forwards,
            error_forwards: d.error_forwards,
            unbound_rejects: d.unbound_rejects,
        })
    }
}

impl Driver for LlmProxyDriver {
    type State = LlmProxyState;

    fn step(&mut self, step: &Step) -> Result {
        match step.action_taken.as_str() {
            "init" | "step" => {
                *self = LlmProxyDriver::default();
            }
            "start_listen" => {
                if !self.listening {
                    self.listening = true;
                }
            }
            "forward_ok" => {
                if self.listening {
                    self.event_seq += 1;
                    self.success_forwards += 1;
                }
            }
            "forward_err" => {
                if self.listening {
                    self.event_seq += 1;
                    self.error_forwards += 1;
                }
            }
            "reject_unbound" => {
                if !self.listening {
                    self.unbound_rejects += 1;
                }
            }
            other => anyhow::bail!("unexpected llm_proxy action: {other}"),
        }
        Ok(())
    }
}

#[quint_run(spec = "specs/llm-proxy.qnt", max_samples = 10, max_steps = 10)]
fn llm_proxy_run() -> impl Driver {
    LlmProxyDriver::default()
}
