// SPDX-License-Identifier: AGPL-3.0-or-later
use super::plan::{GithubRelease, parse_sha256, release_asset_plan, verify_sha256};
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use reqwest::blocking::Client;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tar::Archive;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/marquesds/kaizen/releases/latest";

pub fn install_latest_release() -> Result<()> {
    let exe = std::env::current_exe().context("detect current executable")?;
    let client = github_client()?;
    let release = fetch_latest_release(&client)?;
    let plan = release_asset_plan(&release, std::env::consts::OS, std::env::consts::ARCH)?;
    let archive = fetch_bytes(&client, &plan.archive_url)?;
    let checksum = fetch_text(&client, &plan.checksum_url)?;
    verify_sha256(&archive, &parse_sha256(&checksum)?)?;
    replace_current_binary(&archive, &exe).with_context(
        || "install downloaded release; run `kaizen upgrade --from-source` as fallback",
    )?;
    println!("Installed kaizen {}", plan.version);
    Ok(())
}

fn github_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("kaizen/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build GitHub HTTP client")
}

fn fetch_latest_release(client: &Client) -> Result<GithubRelease> {
    client
        .get(LATEST_RELEASE_URL)
        .send()?
        .error_for_status()?
        .json()
        .context("read latest GitHub release")
}

fn fetch_bytes(client: &Client, url: &str) -> Result<Vec<u8>> {
    Ok(client
        .get(url)
        .send()?
        .error_for_status()?
        .bytes()?
        .to_vec())
}

fn fetch_text(client: &Client, url: &str) -> Result<String> {
    client
        .get(url)
        .send()?
        .error_for_status()?
        .text()
        .context("read release checksum")
}

fn replace_current_binary(archive: &[u8], exe: &Path) -> Result<()> {
    let stage = stage_path(exe)?;
    extract_binary(archive, &stage)?;
    mark_executable(&stage)?;
    std::fs::rename(&stage, exe).with_context(|| format!("replace {}", exe.display()))?;
    Ok(())
}

fn stage_path(exe: &Path) -> Result<PathBuf> {
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("binary has no parent dir"))?;
    Ok(dir.join(format!(".kaizen-upgrade-{}", std::process::id())))
}

fn extract_binary(archive: &[u8], dest: &Path) -> Result<()> {
    let gz = GzDecoder::new(Cursor::new(archive));
    for entry in Archive::new(gz).entries()? {
        let mut entry = entry?;
        if is_kaizen_binary(&entry.path()?) {
            entry.unpack(dest).context("extract kaizen binary")?;
            return Ok(());
        }
    }
    bail!("release archive did not contain a kaizen binary")
}

fn is_kaizen_binary(path: &Path) -> bool {
    path.file_name().and_then(|s| s.to_str()) == Some("kaizen")
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn mark_executable(_path: &Path) -> Result<()> {
    Ok(())
}
