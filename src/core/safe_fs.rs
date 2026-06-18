// SPDX-License-Identifier: AGPL-3.0-or-later
//! File opens that never follow a final symlink on supported platforms.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub fn create_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    no_follow(&mut options);
    options.open(path)
}

pub fn append(path: &Path) -> io::Result<File> {
    reject_alias(path)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    no_follow(&mut options);
    options.open(path)
}

pub fn read_write(path: &Path) -> io::Result<File> {
    reject_alias(path)?;
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    no_follow(&mut options);
    options.open(path)
}

pub fn write_atomic(path: &Path, content: &[u8]) -> io::Result<()> {
    reject_alias(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content)?;
    tmp.as_file().sync_all().ok();
    tmp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

pub fn reject_alias(path: &Path) -> io::Result<()> {
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to write through symlink",
        ));
    }
    reject_hardlink(path)
}

#[cfg(unix)]
pub fn reject_hardlink(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.nlink() > 1 => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to write through hard link",
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
pub fn reject_hardlink(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn no_follow(_options: &mut OpenOptions) {}
