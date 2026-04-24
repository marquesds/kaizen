// SPDX-License-Identifier: AGPL-3.0-or-later
//! Optional cleartext identity fields for session/event payloads (redaction by default; allowlisted in config).

use serde::{Deserialize, Serialize};

/// Labels that may appear on outbound / canonical events when the corresponding
/// `IdentityAllowlist` bit is set in `TelemetryQueryConfig` (or future policy).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorIdentity {
    pub team: Option<String>,
    pub workspace_label: Option<String>,
    pub runner_label: Option<String>,
    pub actor_kind: Option<String>,
    pub actor_label: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub env: Option<String>,
    pub job: Option<String>,
    pub branch: Option<String>,
}

impl ActorIdentity {
    /// All fields empty.
    pub fn is_empty(&self) -> bool {
        self.team.is_none()
            && self.workspace_label.is_none()
            && self.runner_label.is_none()
            && self.actor_kind.is_none()
            && self.actor_label.is_none()
            && self.agent.is_none()
            && self.model.is_none()
            && self.env.is_none()
            && self.job.is_none()
            && self.branch.is_none()
    }
}
