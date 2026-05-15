// SPDX-License-Identifier: AGPL-3.0-or-later
use anyhow::{Result, bail};
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpgradeAction {
    Homebrew,
    ReleaseBinary,
    SourceCargo,
}

#[derive(Debug, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ReleaseAssetPlan {
    pub version: String,
    pub target: String,
    pub archive_name: String,
    pub checksum_name: String,
    pub archive_url: String,
    pub checksum_url: String,
}

pub fn upgrade_action_for(exe: &Path, from_source: bool) -> UpgradeAction {
    if from_source {
        UpgradeAction::SourceCargo
    } else if super::is_homebrew_install(exe) {
        UpgradeAction::Homebrew
    } else {
        UpgradeAction::ReleaseBinary
    }
}

pub fn release_asset_plan(
    release: &GithubRelease,
    os: &str,
    arch: &str,
) -> Result<ReleaseAssetPlan> {
    let target = target_triple(os, arch)?;
    let version = release.tag_name.trim_start_matches('v').to_string();
    let archive_name = format!("kaizen-v{version}-{target}.tar.gz");
    let checksum_name = format!("{archive_name}.sha256");
    Ok(ReleaseAssetPlan {
        archive_url: asset_url(release, &archive_name)?,
        checksum_url: asset_url(release, &checksum_name)?,
        version,
        target,
        archive_name,
        checksum_name,
    })
}

pub fn target_triple(os: &str, arch: &str) -> Result<String> {
    match (os, arch) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin".into()),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin".into()),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu".into()),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu".into()),
        _ => bail!("no binary release for {os}/{arch}; run `kaizen upgrade --from-source`"),
    }
}

pub fn parse_sha256(text: &str) -> Result<String> {
    let hash = text.split_whitespace().next().unwrap_or_default();
    if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(hash.to_ascii_lowercase())
    } else {
        bail!("release checksum asset did not contain a SHA-256 hash")
    }
}

pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = sha256_hex(bytes);
    if actual == expected.to_ascii_lowercase() {
        Ok(())
    } else {
        bail!("checksum mismatch for release binary")
    }
}

fn asset_url(release: &GithubRelease, name: &str) -> Result<String> {
    release
        .assets
        .iter()
        .find(|asset| asset.name == name)
        .map(|asset| asset.browser_download_url.clone())
        .ok_or_else(|| anyhow::anyhow!("release asset missing: {name}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn cargo_home_defaults_to_release_binary() {
        let path = Path::new("/Users/me/.cargo/bin/kaizen");
        assert_eq!(
            upgrade_action_for(path, false),
            UpgradeAction::ReleaseBinary
        );
    }

    #[test]
    fn from_source_uses_cargo_install() {
        let path = Path::new("/Users/me/.cargo/bin/kaizen");
        assert_eq!(upgrade_action_for(path, true), UpgradeAction::SourceCargo);
    }

    #[test]
    fn homebrew_uses_brew_upgrade() {
        let path = Path::new("/opt/homebrew/bin/kaizen");
        assert_eq!(upgrade_action_for(path, false), UpgradeAction::Homebrew);
    }

    #[test]
    fn maps_supported_targets() {
        assert_eq!(
            target_triple("linux", "x86_64").unwrap(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            target_triple("macos", "aarch64").unwrap(),
            "aarch64-apple-darwin"
        );
    }

    #[test]
    fn rejects_unsupported_targets() {
        assert!(target_triple("windows", "x86_64").is_err());
    }

    #[test]
    fn parses_checksum_line() {
        let hash = "0".repeat(64);
        assert_eq!(
            parse_sha256(&format!("{hash}  kaizen.tar.gz")).unwrap(),
            hash
        );
    }

    #[test]
    fn detects_checksum_mismatch() {
        assert!(verify_sha256(b"abc", &"0".repeat(64)).is_err());
    }

    #[test]
    fn finds_release_assets() {
        let release = GithubRelease {
            tag_name: "v1.2.3".into(),
            assets: vec![
                asset("kaizen-v1.2.3-x86_64-unknown-linux-gnu.tar.gz", "archive"),
                asset(
                    "kaizen-v1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256",
                    "sha",
                ),
            ],
        };
        let plan = release_asset_plan(&release, "linux", "x86_64").unwrap();
        assert_eq!(plan.version, "1.2.3");
        assert_eq!(plan.archive_url, "archive");
        assert_eq!(plan.checksum_url, "sha");
    }

    fn asset(name: &str, url: &str) -> GithubAsset {
        GithubAsset {
            name: name.into(),
            browser_download_url: url.into(),
        }
    }
}
