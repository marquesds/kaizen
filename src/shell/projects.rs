// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen projects list` — enumerate registered workspaces on this machine.

use anyhow::Result;
use std::path::Path;

pub fn cmd_projects_list() -> Result<()> {
    let paths = crate::core::machine_registry::list_paths()?;
    if paths.is_empty() {
        println!("no registered projects (run kaizen init inside a workspace first)");
        return Ok(());
    }
    print_table(&paths);
    Ok(())
}

fn print_table(paths: &[std::path::PathBuf]) {
    let header = format!("{:<24}  {:<40}  {}", "NAME", "SLUG", "PATH");
    println!("{header}");
    println!("{}", "-".repeat(header.len()));
    for path in paths {
        let name = segment(path);
        let slug = crate::core::paths::workspace_slug(path);
        println!("{name:<24}  {slug:<40}  {}", path.display());
    }
}

fn segment(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
}
