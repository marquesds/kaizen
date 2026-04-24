// SPDX-License-Identifier: AGPL-3.0-or-later
//! Query-back from telemetry providers (PostHog, Datadog). OTLP has no pull in v1.

use anyhow::Result;
use serde_json::Value;
use std::time::Duration;

#[cfg(feature = "telemetry-datadog")]
mod datadog;
#[cfg(feature = "telemetry-posthog")]
mod posthog;
mod pull_import;

/// Trailing time window for a pull (coarse; provider maps to its API).
#[derive(Debug, Clone, Copy)]
pub struct PullWindow {
    pub days: u32,
}

impl Default for PullWindow {
    fn default() -> Self {
        Self { days: 7 }
    }
}

/// One page of remote rows; cursor is opaque to Kaizen.
#[derive(Debug, Clone, Default)]
pub struct PullPage {
    pub next_cursor: Option<String>,
    pub items: Vec<Value>,
}

pub use pull_import::import_pull_page_to_remote;

/// Abstraction for PostHog / Datadog query APIs. OTLP is export-only, not a query authority.
pub trait TelemetryQueryProvider: Send + Sync {
    fn health(&self) -> Result<()>;
    /// Provider-reported label for debugging (e.g. `posthog-2024-01`).
    fn schema_version(&self) -> &str;
    fn pull(&self, window: PullWindow, cursor: Option<&str>) -> Result<PullPage>;
}

// Submodules `posthog` / `datadog` are feature-gated; keep timeout here for a single value.
#[allow(dead_code)]
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Build a `TelemetryQueryProvider` for the configured query authority, or `None` when pull is off.
pub fn from_config(
    q: &crate::core::config::TelemetryQueryConfig,
) -> Option<std::sync::Arc<dyn TelemetryQueryProvider>> {
    use crate::core::config::QueryAuthority;
    match q.provider {
        QueryAuthority::None => None,
        #[cfg(feature = "telemetry-posthog")]
        QueryAuthority::Posthog => {
            // Real impl needs host/key from env like exporters; minimal stub for compile.
            posthog::posthog_from_env()
        }
        #[cfg(not(feature = "telemetry-posthog"))]
        QueryAuthority::Posthog => {
            tracing::warn!(
                "telemetry query provider is posthog but `telemetry-posthog` feature is off"
            );
            None
        }
        #[cfg(feature = "telemetry-datadog")]
        QueryAuthority::Datadog => datadog::datadog_from_env(),
        #[cfg(not(feature = "telemetry-datadog"))]
        QueryAuthority::Datadog => {
            tracing::warn!(
                "telemetry query provider is datadog but `telemetry-datadog` feature is off"
            );
            None
        }
    }
}

#[allow(dead_code)]
pub(crate) fn http_timeout() -> Duration {
    HTTP_TIMEOUT
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Nop;
    impl TelemetryQueryProvider for Nop {
        fn health(&self) -> Result<()> {
            Ok(())
        }
        fn schema_version(&self) -> &'static str {
            "test"
        }
        fn pull(&self, _window: PullWindow, _cursor: Option<&str>) -> Result<PullPage> {
            Ok(PullPage::default())
        }
    }

    #[test]
    fn nop_pull_empty() {
        let p = Nop;
        assert_eq!(p.schema_version(), "test");
        let page = p.pull(PullWindow { days: 1 }, None).unwrap();
        assert!(page.items.is_empty());
    }
}
