// SPDX-License-Identifier: AGPL-3.0-or-later
pub mod download;
pub mod plan;

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

use plan::{UpgradeAction, upgrade_action_for};

pub fn cmd_upgrade(from_source: bool) -> Result<()> {
    match upgrade_action_for_current_exe(from_source)? {
        UpgradeAction::Homebrew => run_command("brew", &["upgrade", "kaizen-cli"]),
        UpgradeAction::SourceCargo => {
            run_command("cargo", &["install", "kaizen-cli", "--locked", "--force"])
        }
        UpgradeAction::ReleaseBinary => download::install_latest_release(),
    }
}

fn upgrade_action_for_current_exe(from_source: bool) -> Result<UpgradeAction> {
    let exe = std::env::current_exe().context("detect current executable")?;
    Ok(upgrade_action_for(&exe, from_source))
}

fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    println!("Running: {} {}", cmd, args.join(" "));
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        bail!("{cmd} exited with status {status}");
    }
    Ok(())
}

pub fn is_homebrew_install(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.contains("/Cellar/kaizen-cli")
        || path.contains("/opt/homebrew/")
        || path.contains("/usr/local/Cellar/")
}
