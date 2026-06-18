use super::super::WorkspaceFlags;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(clap::Args)]
pub(crate) struct ProjectsCommand {
    #[command(subcommand)]
    pub(crate) subcmd: Option<ProjectsSubcommand>,
    /// Emit a JSON array.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// Include registered paths that no longer exist.
    #[arg(long, global = true)]
    pub(crate) include_missing: bool,
}

#[derive(Subcommand)]
pub(crate) enum ProjectsSubcommand {
    /// List registered workspaces.
    List,
}

#[derive(clap::Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct SearchCommand {
    #[command(subcommand)]
    pub(crate) subcmd: Option<SearchMaintenanceCommand>,
    /// Query indexed session events.
    pub(crate) query: Option<String>,
    #[arg(long)]
    pub(crate) since: Option<String>,
    #[arg(long)]
    pub(crate) agent: Option<String>,
    #[arg(long)]
    pub(crate) kind: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
    #[command(flatten)]
    pub(crate) ws: WorkspaceFlags,
}

#[derive(Subcommand)]
pub(crate) enum SearchMaintenanceCommand {
    /// Drop and rebuild the workspace search index.
    Reindex {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}
