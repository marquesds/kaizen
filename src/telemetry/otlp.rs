// SPDX-License-Identifier: AGPL-3.0-or-later
//! Placeholder for OTLP push; wire a full OpenTelemetry SDK mapping when a collector path is required.

use crate::sync::IngestExportBatch;
use crate::telemetry::TelemetryExporter;
use anyhow::Result;

pub struct OtlpExporter {
    _endpoint: String,
}

impl OtlpExporter {
    pub fn new(endpoint: &str) -> Self {
        Self {
            _endpoint: endpoint.to_string(),
        }
    }
}

impl TelemetryExporter for OtlpExporter {
    fn name(&self) -> &str {
        "otlp"
    }

    fn export(&self, batch: &IngestExportBatch) -> Result<()> {
        tracing::debug!(
            target: "kaizen::telemetry::otlp",
            kind = %batch.kind_name(),
            items = batch.item_count(),
            "OTLP push not fully wired; enable a collector mapping in a follow-up"
        );
        Ok(())
    }
}
