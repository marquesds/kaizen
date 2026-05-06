// SPDX-License-Identifier: AGPL-3.0-or-later
use anyhow::{Result, bail};
use std::process::Command;

pub fn cmd_upgrade() -> Result<()> {
    let (cmd, args) = detect_upgrade_command()?;
    println!("Running: {} {}", cmd, args.join(" "));
    let status = Command::new(cmd).args(&args).status()?;
    if !status.success() {
        bail!("{} exited with status {}", cmd, status);
    }
    Ok(())
}

fn detect_upgrade_command() -> Result<(&'static str, Vec<&'static str>)> {
    let exe = std::env::current_exe()?;
    let path = exe.to_string_lossy().into_owned();
    if is_homebrew_install(&path) {
        Ok(("brew", vec!["upgrade", "kaizen-cli"]))
    } else {
        Ok((
            "cargo",
            vec!["install", "kaizen-cli", "--locked", "--force"],
        ))
    }
}

fn is_homebrew_install(path: &str) -> bool {
    path.contains("/Cellar/kaizen-cli")
        || path.contains("/opt/homebrew/")
        || path.contains("/usr/local/Cellar/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cellar_path_is_homebrew() {
        assert!(is_homebrew_install(
            "/usr/local/Cellar/kaizen-cli/1.0/bin/kaizen"
        ));
    }

    #[test]
    fn opt_homebrew_path_is_homebrew() {
        assert!(is_homebrew_install("/opt/homebrew/bin/kaizen"));
    }

    #[test]
    fn cargo_home_path_is_not_homebrew() {
        assert!(!is_homebrew_install("/Users/me/.cargo/bin/kaizen"));
    }
}
