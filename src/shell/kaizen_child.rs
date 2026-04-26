// SPDX-License-Identifier: AGPL-3.0-or-later
//! Detached `kaizen` child process (ingest outlives parent).

use std::ffi::OsString;
use std::process::{Command, Stdio};

/// Spawn the current binary; never blocks on child.
pub fn spawn_kaizen_detached(args: &[OsString]) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}
