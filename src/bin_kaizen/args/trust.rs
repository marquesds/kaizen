use super::*;

#[derive(Subcommand)]
pub(crate) enum ProjectsCommand {
    /// List registered workspaces.
    List,
}

#[derive(Subcommand)]
pub(crate) enum FeedbackCommand {
    /// List feedback records for the workspace.
    List {
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        #[arg(long, value_enum)]
        label: Option<FeedbackLabel>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub(crate) enum PromptCommand {
    /// List all recorded prompt snapshots.
    List {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Emit JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Show files in a snapshot by fingerprint prefix.
    Show {
        fingerprint: String,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },
    /// Diff two snapshots.
    Diff {
        fingerprint_a: String,
        fingerprint_b: String,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum SessionsCommand {
    /// List sessions for current workspace.
    List {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
        /// Emit JSON (same as MCP with json=true)
        #[arg(long)]
        json: bool,
        /// Cap rows after sorting (newest first). Omit for 100 rows; 0 returns all.
        #[arg(long)]
        limit: Option<usize>,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
    },
    /// Load previous local agent sessions into Kaizen stores.
    Load {
        /// workspace root; omit to load all registered workspaces
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Emit JSON load summary.
        #[arg(long)]
        json: bool,
    },
    /// Show full details for a session.
    Show {
        id: String,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Attach human feedback (score/label/note) to a session.
    Annotate {
        id: String,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=5))]
        score: Option<u8>,
        #[arg(long, value_enum)]
        label: Option<FeedbackLabel>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Render nested tool-span tree for a session.
    Tree {
        id: String,
        #[arg(long, default_value = "999")]
        depth: u32,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Render Datadog-style trace spans for a session.
    Trace {
        id: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Full-text search session events.
    Search {
        query: String,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum SearchCommand {
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

#[derive(Subcommand)]
pub(crate) enum OutcomesCommand {
    /// Show stored JSON row for a session.
    Show {
        id: String,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Internal: run tests/lint and upsert `session_outcomes` (ingest spawns this).
    #[command(hide = true)]
    Measure {
        /// workspace root (db + repo path)
        #[arg(long)]
        workspace: PathBuf,
        /// Session id
        #[arg(long)]
        session: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum MetricsCommand {
    /// Rebuild repo snapshot and Ladybug sidecar.
    Index {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Rebuild even when fingerprint unchanged.
        #[arg(long)]
        force: bool,
    },
    /// Field coverage and trace-correlation health.
    Quality {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Trailing window in days.
        #[arg(long, default_value_t = 7)]
        days: u32,
        /// Emit JSON report.
        #[arg(long)]
        json: bool,
    },
}
