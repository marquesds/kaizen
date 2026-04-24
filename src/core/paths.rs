// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared path helpers (used by `workspace` and `machine_registry` to avoid import cycles).

use std::path::{Path, PathBuf};

/// `KAIZEN_HOME` or `~/.kaizen` (requires `HOME`), or `None` if undiscoverable.
pub fn kaizen_dir() -> Option<PathBuf> {
    std::env::var("KAIZEN_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".kaizen"))
        })
}

pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| absolute(path))
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
pub(crate) mod test_lock {
    use std::sync::{Mutex, OnceLock};

    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    pub fn global() -> &'static Mutex<()> {
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
