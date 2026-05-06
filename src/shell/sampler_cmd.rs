// SPDX-License-Identifier: AGPL-3.0-or-later
//! Detached process sampler (`__sampler-run`).

use crate::core::config;
use crate::store::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

fn stop_path(workspace: &Path, session_id: &str) -> Result<PathBuf> {
    Ok(crate::core::paths::project_data_dir(workspace)?
        .join("sampler-stop")
        .join(session_id))
}

/// Sample `pid` until stop file, cap, or process exit.
pub fn cmd_sampler_run(workspace: &Path, session_id: &str, pid: u32) -> Result<()> {
    let cfg = config::load(workspace)?;
    let s = &cfg.collect.system_sampler;
    if !s.enabled {
        return Ok(());
    }
    let store = Store::open(&crate::core::workspace::db_path(workspace)?)?;
    let target = Pid::from_u32(pid);
    let mut sys = System::new();
    let kind = ProcessRefreshKind::nothing().with_cpu().with_memory();
    let mut n: u32 = 0;
    while n < s.max_samples_per_session {
        if stop_path(workspace, session_id).is_ok_and(|p| p.exists()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(s.sample_ms.max(100)));
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[target]), false, kind);
        let ts = now_ms();
        let row = sys.process(target);
        let (cpu, rss) = match row {
            Some(p) => (Some(p.cpu_usage() as f64), Some(p.memory())),
            None => break,
        };
        store.append_session_sample(session_id, ts, pid, cpu, rss)?;
        n += 1;
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
