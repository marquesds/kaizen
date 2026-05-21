use crate::bin_kaizen::args::WorkspaceFlags;
use crate::bin_kaizen::workspace::resolve_ws;
use std::path::PathBuf;

pub(super) fn ws(flags: &WorkspaceFlags) -> anyhow::Result<Option<PathBuf>> {
    resolve_ws(flags.workspace.as_deref(), flags.project.as_deref())
}
