// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result, anyhow, bail};
use rand::Rng;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

const TOKEN_BYTES: usize = 32;
const TOKEN_HEX_LEN: usize = TOKEN_BYTES * 2;

pub(super) fn ephemeral() -> String {
    let mut bytes = [0_u8; TOKEN_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub(super) fn load_or_create_at(path: &Path) -> Result<String> {
    crate::core::safe_fs::reject_alias(path)?;
    create_parent(path)?;
    match load(path)? {
        Some(token) => Ok(token),
        None => create(path),
    }
}

fn create_parent(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("web token path has no parent"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("create web token directory: {}", parent.display()))
}

fn load(path: &Path) -> Result<Option<String>> {
    crate::core::safe_fs::reject_alias(path)?;
    match fs::read_to_string(path) {
        Ok(raw) => validate_and_secure(path, raw).map(Some),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("read web token: {}", path.display())),
    }
}

fn validate_and_secure(path: &Path, raw: String) -> Result<String> {
    let token = validate(path, raw)?;
    set_private(path)?;
    Ok(token)
}

fn validate(path: &Path, raw: String) -> Result<String> {
    if raw.len() != TOKEN_HEX_LEN || !raw.bytes().all(is_lower_hex) {
        bail!(
            "invalid web token at {}: expected 64 lowercase hex characters",
            path.display()
        );
    }
    Ok(raw)
}

fn create(path: &Path) -> Result<String> {
    let token = ephemeral();
    let temp = write_temp(path, &token)?;
    install(path, temp, token)
}

fn write_temp(path: &Path, token: &str) -> Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("web token path has no parent"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    set_private(temp.path())?;
    temp.write_all(token.as_bytes())?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

fn install(path: &Path, temp: tempfile::NamedTempFile, token: String) -> Result<String> {
    match temp.persist_noclobber(path) {
        Ok(_) => Ok(token),
        Err(err) if err.error.kind() == ErrorKind::AlreadyExists => load_required(path),
        Err(err) => {
            Err(err.error).with_context(|| format!("persist web token: {}", path.display()))
        }
    }
}

fn load_required(path: &Path) -> Result<String> {
    load(path)?.ok_or_else(|| anyhow!("web token disappeared during creation: {}", path.display()))
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

#[cfg(unix)]
fn set_private(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("set web token permissions: {}", path.display()))
}

#[cfg(not(unix))]
fn set_private(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ephemeral, load_or_create_at};
    use std::sync::{Arc, Barrier};

    #[test]
    fn ephemeral_token_is_32_random_bytes_as_lowercase_hex() {
        let token = ephemeral();
        assert_eq!(token.len(), 64);
        assert!(token.bytes().all(is_lower_hex));
    }

    #[test]
    fn persisted_token_is_reused() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("web_token.hex");
        let first = load_or_create_at(&path).unwrap();
        let second = load_or_create_at(&path).unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::read_to_string(path).unwrap(), first);
    }

    #[test]
    fn malformed_token_fails_without_rotation() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("web_token.hex");
        std::fs::write(&path, "ABC123").unwrap();
        assert!(load_or_create_at(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "ABC123");
    }

    #[test]
    fn unreadable_token_path_fails_without_replacement() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("web_token.hex");
        std::fs::create_dir(&path).unwrap();
        assert!(load_or_create_at(&path).is_err());
        assert!(path.is_dir());
    }

    #[test]
    fn concurrent_creation_reuses_one_winner() {
        let home = tempfile::tempdir().unwrap();
        let path = Arc::new(home.path().join("web_token.hex"));
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| spawn_load(path.clone(), barrier.clone()))
            .collect::<Vec<_>>();
        let tokens = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(tokens.iter().all(|token| token == &tokens[0]));
    }

    #[cfg(unix)]
    #[test]
    fn loading_token_sets_unix_mode_to_0600() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("web_token.hex");
        std::fs::write(&path, "ab".repeat(32)).unwrap();
        std::fs::set_permissions(&path, PermissionsExt::from_mode(0o644)).unwrap();
        load_or_create_at(&path).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    fn spawn_load(
        path: Arc<std::path::PathBuf>,
        barrier: Arc<Barrier>,
    ) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            barrier.wait();
            load_or_create_at(&path).unwrap()
        })
    }

    fn is_lower_hex(byte: u8) -> bool {
        byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
    }
}
