use super::*;

#[derive(Subcommand)]
pub(crate) enum SyncCommand {
    /// Run sync loop until interrupted (or use --once).
    Run {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Single flush then exit (for tests / scripts).
        #[arg(long)]
        once: bool,
    },
    /// Show outbox depth and last flush / error state.
    Status {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum DaemonCommand {
    /// Run daemon in foreground, or spawn it and exit with `--background`.
    Start {
        /// Spawn daemon as child process, wait until ready, then exit.
        #[arg(long)]
        background: bool,
    },
    /// Gracefully stop daemon.
    Stop,
    /// Show daemon pid, uptime, queue depth, and last error.
    Status,
}

#[derive(Subcommand)]
pub(crate) enum IngestCommand {
    /// Read hook event from stdin and log it.
    Hook {
        /// hook source agent
        #[arg(long, value_enum)]
        source: Source,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}
