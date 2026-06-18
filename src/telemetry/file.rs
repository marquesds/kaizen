// SPDX-License-Identifier: AGPL-3.0-or-later
//! Append-only NDJSON file sink (local telemetry).

use super::TelemetryExporter;
use super::batch_metadata;
use crate::sync::IngestExportBatch;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Default: `$KAIZEN_HOME/projects/<slug>/telemetry.ndjson`.
pub fn default_ndjson_path(workspace: &Path) -> Result<PathBuf> {
    crate::core::paths::project_data_child(workspace, Path::new("telemetry.ndjson"))
}

pub fn resolve_file_exporter_path(path_opt: Option<&str>, workspace: &Path) -> Result<PathBuf> {
    let Some(p) = path_opt.map(PathBuf::from) else {
        return default_ndjson_path(workspace);
    };
    if p.is_absolute() {
        ensure_outside_workspace(&p, workspace)?;
        ensure_not_symlink(&p)?;
        return Ok(p);
    }
    ensure_relative(&p)?;
    crate::core::paths::project_data_child(workspace, &p)
}

fn ensure_not_symlink(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "telemetry output rejects symlink"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn ensure_relative(path: &Path) -> Result<()> {
    let escapes = path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir));
    anyhow::ensure!(!escapes, "telemetry file path cannot contain `..`");
    Ok(())
}

fn ensure_outside_workspace(path: &Path, workspace: &Path) -> Result<()> {
    let workspace = crate::core::paths::canonical(workspace);
    let linked_inside = path
        .ancestors()
        .find_map(|parent| parent.canonicalize().ok())
        .is_some_and(|parent| parent.starts_with(&workspace));
    anyhow::ensure!(
        !path.starts_with(&workspace) && !linked_inside,
        "telemetry output cannot be inside target repository"
    );
    Ok(())
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
    ensure_not_symlink(path)?;
    let mut f =
        crate::core::safe_fs::append(path).with_context(|| format!("open {}", path.display()))?;
    serde_json::to_writer(&mut f, v)?;
    f.write_all(b"\n")?;
    f.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::test_lock;
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

    #[test]
    fn relative_path_stays_in_project_data() {
        let _guard = test_lock::global().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
        let path =
            resolve_file_exporter_path(Some("custom/events.ndjson"), workspace.path()).unwrap();
        let expected = crate::core::paths::project_data_path(workspace.path())
            .unwrap()
            .join("custom/events.ndjson");
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        assert_eq!(path, expected);
    }

    #[test]
    fn absolute_path_inside_workspace_is_rejected() {
        let workspace = tempfile::tempdir().unwrap();
        let path = workspace.path().join("telemetry.ndjson");
        let error = resolve_file_exporter_path(path.to_str(), workspace.path()).unwrap_err();
        assert!(error.to_string().contains("target repository"));
    }
}
