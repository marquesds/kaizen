// SPDX-License-Identifier: AGPL-3.0-or-later
//! Placeholder for OTLP push; wire a full OpenTelemetry SDK mapping when a collector path is required.

use crate::sync::IngestExportBatch;
use crate::sync::canonical::expand_ingest_batch;
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
        let items = expand_ingest_batch(batch);
        tracing::debug!(
            target: "kaizen::telemetry::otlp",
            kind = %batch.kind_name(),
            batch_items = batch.item_count(),
            canonical_items = items.len(),
            "OTLP push not fully wired; canonical item count logged for parity with other exporters"
        );
        Ok(())
    }
}
