// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::collect::hooks::{EventKind, HookEvent};
use crate::core::config;
use anyhow::Result;
use serde_json::Value;
use std::ffi::OsString;
use std::path::Path;

pub(super) fn post_ingest_detached(
    event: &HookEvent,
    cfg: &config::Config,
    workspace: &Path,
) -> Result<()> {
    if matches!(event.kind, EventKind::Stop) {
        stop_sidecars(event, cfg, workspace);
    }
    if should_start_sampler(event, cfg)
        && let Some(pid) = payload_pid(&event.payload)
    {
        spawn_sampler_run(workspace, &event.session_id, pid);
    }
    Ok(())
}

fn stop_sidecars(event: &HookEvent, cfg: &config::Config, workspace: &Path) {
    if cfg.collect.outcomes.enabled {
        spawn_outcome_measure(workspace, &event.session_id);
    }
    if cfg.collect.system_sampler.enabled {
        touch_sampler_stop_file(workspace, &event.session_id);
    }
}

fn should_start_sampler(event: &HookEvent, cfg: &config::Config) -> bool {
    matches!(event.kind, EventKind::SessionStart) && cfg.collect.system_sampler.enabled
}

fn payload_pid(value: &Value) -> Option<u32> {
    value
        .get("pid")
        .and_then(|pid| pid.as_u64().map(|raw| raw as u32))
        .or_else(|| {
            value
                .get("pid")
                .and_then(Value::as_i64)
                .and_then(|raw| u32::try_from(raw).ok())
        })
}

fn spawn_outcome_measure(workspace: &Path, session_id: &str) {
    let args = vec![
        OsString::from("outcomes"),
        OsString::from("measure"),
        OsString::from("--workspace"),
        workspace.as_os_str().to_owned(),
        OsString::from("--session"),
        OsString::from(session_id),
    ];
    spawn_detached(args, "kaizen outcomes measure");
}

fn spawn_sampler_run(workspace: &Path, session_id: &str, pid: u32) {
    let args = vec![
        OsString::from("__sampler-run"),
        OsString::from("--workspace"),
        workspace.as_os_str().to_owned(),
        OsString::from("--session"),
        OsString::from(session_id),
        OsString::from("--pid"),
        OsString::from(pid.to_string()),
    ];
    spawn_detached(args, "kaizen sampler");
}

fn spawn_detached(args: Vec<OsString>, message: &str) {
    if let Err(error) = super::super::kaizen_child::spawn_kaizen_detached(&args) {
        tracing::warn!(?error, "{message}");
    }
}

fn touch_sampler_stop_file(workspace: &Path, session_id: &str) {
    let Some(dir) = stop_dir(workspace) else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(&dir) {
        tracing::warn!(?error, "sampler-stop mkdir");
        return;
    }
    if let Err(error) = std::fs::File::create(dir.join(session_id)) {
        tracing::warn!(?error, "sampler-stop touch");
    }
}

fn stop_dir(workspace: &Path) -> Option<std::path::PathBuf> {
    crate::core::paths::project_data_dir(workspace)
        .map(|dir| dir.join("sampler-stop"))
        .map_err(|error| tracing::warn!(?error, "sampler-stop: no data dir"))
        .ok()
}
