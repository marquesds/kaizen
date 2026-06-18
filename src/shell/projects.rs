// SPDX-License-Identifier: AGPL-3.0-or-later
//! Registered workspace listing.

use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct ProjectRow {
    name: String,
    slug: String,
    path: String,
    status: &'static str,
}

pub fn cmd_projects_list(json: bool, include_missing: bool) -> Result<()> {
    print!("{}", projects_text(json, include_missing)?);
    Ok(())
}

pub fn projects_text(json: bool, include_missing: bool) -> Result<String> {
    let rows = project_rows(include_missing)?;
    if json {
        return Ok(format!("{}\n", serde_json::to_string_pretty(&rows)?));
    }
    Ok(render_table(&rows))
}

fn project_rows(include_missing: bool) -> Result<Vec<ProjectRow>> {
    let paths = if include_missing {
        crate::core::machine_registry::list_paths_including_missing()?
    } else {
        crate::core::machine_registry::list_paths()?
    };
    Ok(paths.into_iter().map(project_row).collect())
}

fn project_row(path: PathBuf) -> ProjectRow {
    ProjectRow {
        name: segment(&path).to_string(),
        slug: crate::core::paths::workspace_slug(&path),
        status: status(&path),
        path: path.to_string_lossy().to_string(),
    }
}

fn status(path: &Path) -> &'static str {
    if path.is_dir() {
        "available"
    } else {
        "missing"
    }
}

fn render_table(rows: &[ProjectRow]) -> String {
    if rows.is_empty() {
        return "no registered projects (run kaizen init inside a workspace first)\n".into();
    }
    let header = format!("{:<24}  {:<40}  {:<10}  PATH", "NAME", "SLUG", "STATUS");
    let body = rows.iter().map(table_row).collect::<Vec<_>>().join("\n");
    format!("{header}\n{}\n{body}\n", "-".repeat(header.len()))
}

fn table_row(row: &ProjectRow) -> String {
    format!(
        "{:<24}  {:<40}  {:<10}  {}",
        row.name, row.slug, row.status, row.path
    )
}

fn segment(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("?")
}
