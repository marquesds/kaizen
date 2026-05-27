// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::guidance::types::{Artifact, ArtifactKind, ArtifactRef};
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};

pub fn scan(workspace: &Path) -> Result<Vec<Artifact>> {
    let mut out = Vec::new();
    out.extend(scan_skills(workspace)?);
    out.extend(scan_rules(workspace)?);
    out.sort_by(|a, b| (a.kind, &a.slug).cmp(&(b.kind, &b.slug)));
    Ok(out)
}

pub fn find(workspace: &Path, artifact: &ArtifactRef) -> Result<Artifact> {
    scan(workspace)?
        .into_iter()
        .find(|a| a.kind == artifact.kind && a.slug == artifact.slug)
        .ok_or_else(|| anyhow!("artifact not found: {artifact}"))
}

fn scan_skills(workspace: &Path) -> Result<Vec<Artifact>> {
    let dir = workspace.join(".cursor/skills");
    scan_dirs(workspace, &dir, ArtifactKind::Skill)
}

fn scan_rules(workspace: &Path) -> Result<Vec<Artifact>> {
    let dir = workspace.join(".cursor/rules");
    scan_files(workspace, &dir, "mdc", ArtifactKind::Rule)
}

fn scan_dirs(workspace: &Path, dir: &Path, kind: ArtifactKind) -> Result<Vec<Artifact>> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| artifact_from_skill_dir(workspace, e.path(), kind).transpose())
        .collect()
}

fn scan_files(
    workspace: &Path,
    dir: &Path,
    ext: &str,
    kind: ArtifactKind,
) -> Result<Vec<Artifact>> {
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some(ext))
        .map(|p| artifact_from_file(workspace, p, kind))
        .collect()
}

fn artifact_from_skill_dir(
    workspace: &Path,
    dir: PathBuf,
    kind: ArtifactKind,
) -> Result<Option<Artifact>> {
    let path = dir.join("SKILL.md");
    if path.is_file() {
        return artifact_from_file(workspace, path, kind).map(Some);
    }
    Ok(None)
}

fn artifact_from_file(workspace: &Path, path: PathBuf, kind: ArtifactKind) -> Result<Artifact> {
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let meta = fs::metadata(&path)?;
    Ok(Artifact {
        kind,
        slug: slug_for(workspace, &path, kind)?,
        path,
        content_hash: hex::encode(blake3::hash(&bytes).as_bytes()),
        bytes: bytes.len() as u64,
        mtime_ms: mtime_ms(&meta),
    })
}

fn slug_for(workspace: &Path, path: &Path, kind: ArtifactKind) -> Result<String> {
    match kind {
        ArtifactKind::Skill => path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("bad skill path: {}", path.display())),
        ArtifactKind::Rule => path
            .strip_prefix(workspace.join(".cursor/rules"))?
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string)
            .ok_or_else(|| anyhow!("bad rule path: {}", path.display())),
    }
}

fn mtime_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
