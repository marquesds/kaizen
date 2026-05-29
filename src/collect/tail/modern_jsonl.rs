// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::core::event::{Event, SessionRecord};
use anyhow::Result;
use std::path::{Path, PathBuf};

type Scanner = fn(&Path) -> Result<(SessionRecord, Vec<Event>)>;
pub(crate) use super::modern_jsonl_event::parse_common_line;
pub(crate) use super::modern_jsonl_record::scan_agent_session_file;

pub fn scan_workspace(
    workspace: &Path,
    env_var: &str,
    home_suffix: &str,
    local_dir: &str,
    scanner: Scanner,
) -> Vec<(SessionRecord, Vec<Event>)> {
    roots(workspace, env_var, home_suffix, local_dir)
        .into_iter()
        .flat_map(jsonl_files)
        .filter_map(|path| scanner(&path).ok())
        .filter(|(session, events)| workspace_match(session, workspace) && !events.is_empty())
        .collect()
}

fn roots(workspace: &Path, env_var: &str, home_suffix: &str, local_dir: &str) -> Vec<PathBuf> {
    [
        std::env::var(env_var).ok().map(PathBuf::from),
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(home_suffix)),
        Some(workspace.join(local_dir)),
    ]
    .into_iter()
    .flatten()
    .filter(|path| path.is_dir())
    .collect()
}

fn jsonl_files(root: PathBuf) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .flat_map(|path| {
            if path.is_dir() {
                jsonl_files(path)
            } else {
                vec![path]
            }
        })
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect()
}

fn workspace_match(session: &SessionRecord, workspace: &Path) -> bool {
    session.workspace.is_empty()
        || std::fs::canonicalize(&session.workspace)
            .ok()
            .is_some_and(|path| path == canonical(workspace))
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
