// SPDX-License-Identifier: AGPL-3.0-or-later
//! Append-only NDJSON file sink (local telemetry).

use super::TelemetryExporter;
use super::batch_metadata;
use crate::sync::IngestExportBatch;
use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Default: `~/.kaizen/projects/<slug>/telemetry.ndjson`.
pub fn default_ndjson_path(workspace: &Path) -> PathBuf {
    crate::core::paths::project_data_dir(workspace)
        .map(|d| d.join("telemetry.ndjson"))
        .unwrap_or_else(|_| workspace.join("telemetry.ndjson"))
}

pub fn resolve_file_exporter_path(path_opt: Option<&str>, workspace: &Path) -> PathBuf {
    let p: PathBuf = path_opt
        .map(PathBuf::from)
        .unwrap_or_else(|| default_ndjson_path(workspace));
    if p.is_absolute() {
        p
    } else {
        workspace.join(p)
    }
}

pub struct FileExporter {
    path: PathBuf,
}

impl FileExporter {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl TelemetryExporter for FileExporter {
    fn name(&self) -> &str {
        "file"
    }

    fn export(&self, batch: &IngestExportBatch) -> Result<()> {
        let t = now_ms();
        let v = batch_metadata::telemetry_file_line(batch, t);
        append_json_line(&self.path, &v)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn append_json_line(path: &Path, v: &serde_json::Value) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("create {}", d.display()))?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;
    serde_json::to_writer(&mut f, v)?;
    f.write_all(b"\n")?;
    f.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::IngestExportBatch;
    use crate::sync::export_batch::SessionEvalsBatchBody;

    #[test]
    fn file_export_writes_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.ndjson");
        let e = FileExporter::new(p.clone());
        let b = IngestExportBatch::SessionEvals(SessionEvalsBatchBody { evals: vec![] });
        e.export(&b).unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(s.lines().next().unwrap()).unwrap();
        assert_eq!(
            v.get("batch_kind").and_then(|x| x.as_str()),
            Some("session_evals")
        );
    }
}
