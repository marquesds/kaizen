use super::*;

#[derive(clap::Args)]
pub(crate) struct WorkspaceFlags {
    #[arg(long)]
    pub(crate) workspace: Option<PathBuf>,
    #[arg(long, conflicts_with = "workspace")]
    pub(crate) project: Option<String>,
}

#[derive(clap::Args)]
pub(crate) struct SharedSinceJson {
    #[arg(long)]
    pub(crate) since: Option<String>,
    #[arg(long)]
    pub(crate) json: bool,
    #[command(flatten)]
    pub(crate) ws: WorkspaceFlags,
}

#[derive(clap::Args)]
pub(crate) struct IdJson {
    pub(crate) id: String,
    #[arg(long)]
    pub(crate) json: bool,
    #[command(flatten)]
    pub(crate) ws: WorkspaceFlags,
}

#[derive(clap::Args)]
pub(crate) struct IdOnly {
    pub(crate) id: String,
    #[command(flatten)]
    pub(crate) ws: WorkspaceFlags,
}

#[derive(clap::Args)]
pub(crate) struct JsonOnly {
    #[arg(long)]
    pub(crate) json: bool,
    #[command(flatten)]
    pub(crate) ws: WorkspaceFlags,
}

#[derive(ValueEnum, Clone, Debug)]
pub(crate) enum Source {
    Cursor,
    Claude,
    Vibe,
}

/// Shells supported by clap_complete (redirect stdout to a file, or eval).
#[derive(Copy, Clone, Debug, ValueEnum, Eq, PartialEq)]
pub(crate) enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Powershell,
    Zsh,
}
