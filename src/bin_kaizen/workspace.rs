use std::path::{Path, PathBuf};

pub(crate) fn resolve_ws(
    workspace: Option<&Path>,
    project: Option<&str>,
) -> anyhow::Result<Option<PathBuf>> {
    match (workspace, project) {
        (None, None) => Ok(None),
        (w, p) => kaizen::shell::cli::resolve_target(w, p).map(|(path, _)| Some(path)),
    }
}
