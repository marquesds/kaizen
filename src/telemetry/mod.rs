// SPDX-License-Identifier: AGPL-3.0-or-later
//! Optional pluggable sinks that receive the same redacted [`IngestExportBatch`] as Kaizen sync.
//! Fan-out runs in parallel with the primary `POST` (see `sync::engine`); outbox is committed only
//! when the primary succeeds (and, when `fail_open` is `false`, when the fan-out completes `Ok`).

mod resolve;

#[cfg(feature = "telemetry-datadog")]
mod datadog;
#[cfg(feature = "telemetry-dev")]
mod dev;
#[cfg(feature = "telemetry-otlp")]
mod otlp;
#[cfg(feature = "telemetry-posthog")]
mod posthog;

use crate::core::config::{ExporterConfig, TelemetryConfig};
use crate::sync::IngestExportBatch;
use anyhow::Result;
use std::sync::Arc;

pub use resolve::DatadogResolved;
pub use resolve::OtlpResolved;
pub use resolve::PostHogResolved;

/// Third-party and OTel sinks use the same batch types as the HTTP ingest.
pub trait TelemetryExporter: Send + Sync {
    fn name(&self) -> &str;
    fn export(&self, batch: &IngestExportBatch) -> Result<()>;
}

/// Built from `TelemetryConfig` via [`load_exporters`]. Empty is a no-op.
pub struct ExporterRegistry {
    exporters: Vec<Arc<dyn TelemetryExporter>>,
}

impl ExporterRegistry {
    pub fn empty() -> Self {
        Self {
            exporters: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.exporters.is_empty()
    }

    pub fn from_vec(exporters: Vec<Arc<dyn TelemetryExporter>>) -> Self {
        Self { exporters }
    }

    /// When `fail_open` is `true`, log each exporter error and continue. If `false`, return the first error.
    pub fn fan_out(&self, fail_open: bool, batch: &IngestExportBatch) -> Result<()> {
        for e in &self.exporters {
            let r = e.export(batch);
            if let Err(err) = r {
                tracing::warn!(exporter = e.name(), error = %err, "telemetry exporter");
                if !fail_open {
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

/// Build exporters from TOML + environment. Missing creds for a sink log a warning and skip it.
pub fn load_exporters(cfg: &TelemetryConfig) -> ExporterRegistry {
    let mut v: Vec<Arc<dyn TelemetryExporter>> = Vec::new();
    for entry in &cfg.exporters {
        if let Some(exp) = build_exporter(entry) {
            v.push(exp);
        }
    }
    ExporterRegistry::from_vec(v)
}

fn build_exporter(c: &ExporterConfig) -> Option<Arc<dyn TelemetryExporter>> {
    if !c.is_enabled() {
        return None;
    }
    match c {
        ExporterConfig::None => None,
        ExporterConfig::Dev { .. } => {
            #[cfg(feature = "telemetry-dev")]
            {
                return Some(Arc::new(dev::DevExporter) as _);
            }
            #[cfg(not(feature = "telemetry-dev"))]
            {
                tracing::warn!(
                    "telemetry `dev` exporter configured but `telemetry-dev` is not enabled"
                );
                None
            }
        }
        ExporterConfig::PostHog { .. } => {
            let r = PostHogResolved::from_config(c)?;
            #[cfg(feature = "telemetry-posthog")]
            {
                return Some(
                    Arc::new(posthog::PostHogExporter::new(&r.host, &r.project_api_key)) as _,
                );
            }
            #[cfg(not(feature = "telemetry-posthog"))]
            {
                let _ = &r;
                tracing::warn!(
                    "PostHog configured but the `telemetry-posthog` feature is not enabled"
                );
                None
            }
        }
        ExporterConfig::Datadog { .. } => {
            let r = DatadogResolved::from_config(c)?;
            #[cfg(feature = "telemetry-datadog")]
            {
                return Some(Arc::new(datadog::DatadogExporter::new(&r.site, &r.api_key)) as _);
            }
            #[cfg(not(feature = "telemetry-datadog"))]
            {
                let _ = &r;
                tracing::warn!(
                    "Datadog configured but the `telemetry-datadog` feature is not enabled"
                );
                None
            }
        }
        ExporterConfig::Otlp { .. } => {
            let r = OtlpResolved::from_config(c)?;
            #[cfg(feature = "telemetry-otlp")]
            {
                return Some(Arc::new(otlp::OtlpExporter::new(&r.endpoint)) as _);
            }
            #[cfg(not(feature = "telemetry-otlp"))]
            {
                let _ = &r;
                tracing::warn!("OTLP configured but the `telemetry-otlp` feature is not enabled");
                None
            }
        }
    }
}
