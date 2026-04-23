// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tracing-only echo exporter (testing / wiring validation).

use crate::sync::IngestExportBatch;
use crate::telemetry::TelemetryExporter;
use anyhow::Result;

pub struct DevExporter;

impl TelemetryExporter for DevExporter {
    fn name(&self) -> &str {
        "dev"
    }

    fn export(&self, batch: &IngestExportBatch) -> Result<()> {
        tracing::info!(
            target: "kaizen::telemetry::dev",
            kind = %batch.kind_name(),
            items = batch.item_count(),
            "telemetry dev exporter"
        );
        Ok(())
    }
}
