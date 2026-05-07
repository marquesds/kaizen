// SPDX-License-Identifier: AGPL-3.0-or-later
//! Query-back from telemetry providers (PostHog, Datadog). OTLP has no pull in v1.

use anyhow::Result;
use serde_json::Value;
use std::time::Duration;

#[cfg(feature = "telemetry-datadog")]
pub(crate) mod datadog;
#[cfg(feature = "telemetry-posthog")]
pub(crate) mod posthog;
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

/// Build a `TelemetryQueryProvider` for the configured query authority, or `None` when pull
/// is off. Resolves credentials from the matching `[[telemetry.exporters]]` row first, then
/// env. This way `kaizen telemetry configure --type=datadog` is enough to drive `pull` —
/// users do not have to *also* `export DD_API_KEY` in their shell.
pub fn from_config(
    cfg: &crate::core::config::TelemetryConfig,
) -> Option<std::sync::Arc<dyn TelemetryQueryProvider>> {
    use crate::core::config::QueryAuthority;
    match cfg.query.provider {
        QueryAuthority::None => None,
        #[cfg(feature = "telemetry-posthog")]
        QueryAuthority::Posthog => posthog_provider(cfg),
        #[cfg(not(feature = "telemetry-posthog"))]
        QueryAuthority::Posthog => {
            tracing::warn!(
                "telemetry query provider is posthog but `telemetry-posthog` feature is off"
            );
            None
        }
        #[cfg(feature = "telemetry-datadog")]
        QueryAuthority::Datadog => datadog_provider(cfg),
        #[cfg(not(feature = "telemetry-datadog"))]
        QueryAuthority::Datadog => {
            tracing::warn!(
                "telemetry query provider is datadog but `telemetry-datadog` feature is off"
            );
            None
        }
    }
}

#[cfg(feature = "telemetry-datadog")]
fn datadog_provider(
    cfg: &crate::core::config::TelemetryConfig,
) -> Option<std::sync::Arc<dyn TelemetryQueryProvider>> {
    use crate::core::config::ExporterConfig;
    let row = cfg
        .exporters
        .iter()
        .find(|e| matches!(e, ExporterConfig::Datadog { .. }));
    let resolved = row
        .and_then(crate::telemetry::DatadogResolved::from_config)
        .or_else(crate::telemetry::DatadogResolved::from_env_only)?;
    Some(std::sync::Arc::new(datadog::DatadogQueryClient::new(&resolved)) as _)
}

#[cfg(feature = "telemetry-posthog")]
fn posthog_provider(
    cfg: &crate::core::config::TelemetryConfig,
) -> Option<std::sync::Arc<dyn TelemetryQueryProvider>> {
    use crate::core::config::ExporterConfig;
    let row = cfg
        .exporters
        .iter()
        .find(|e| matches!(e, ExporterConfig::PostHog { .. }));
    let resolved = row
        .and_then(crate::telemetry::PostHogResolved::from_config)
        .or_else(crate::telemetry::PostHogResolved::from_env_only)?;
    Some(std::sync::Arc::new(posthog::PostHogQueryClient::new(&resolved)) as _)
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
