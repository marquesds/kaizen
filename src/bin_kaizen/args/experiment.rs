use super::*;

#[derive(Subcommand)]
pub(crate) enum ExpCommand {
    /// Create experiment in Draft state (records control/treatment commits).
    New {
        #[arg(long)]
        name: String,
        #[arg(long)]
        hypothesis: String,
        #[arg(long)]
        change: String,
        /// tokens_per_session|cost_per_session|success_rate|tool_loops|duration_minutes|files_per_session
        #[arg(long)]
        metric: String,
        /// git|branch|manual
        #[arg(long, default_value = "git")]
        bind: String,
        #[arg(long, default_value_t = 14)]
        duration_days: u32,
        /// target delta pct, e.g. -10.0 for -10%
        #[arg(long, default_value_t = -10.0, allow_hyphen_values = true)]
        target_pct: f64,
        #[arg(long)]
        control_commit: Option<String>,
        #[arg(long)]
        treatment_commit: Option<String>,
        #[arg(long)]
        control_branch: Option<String>,
        #[arg(long)]
        treatment_branch: Option<String>,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Transition experiment from Draft to Running.
    Start {
        id: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// List all experiments.
    List {
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Show one experiment's metadata.
    Status {
        id: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Manual variant tag for a session.
    Tag {
        id: String,
        #[arg(long)]
        session: String,
        /// control|treatment|excluded
        #[arg(long)]
        variant: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Render markdown (or JSON) report with bootstrap CI.
    Report {
        id: String,
        #[arg(long)]
        json: bool,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Mark experiment Concluded.
    Conclude {
        id: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Mark experiment Archived (must be Concluded first).
    Archive {
        id: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Print MDE at 80% power / 95% CI for a metric given expected sample size.
    Power {
        /// tokens_per_session|cost_per_session|success_rate|…
        #[arg(long)]
        metric: String,
        /// Expected sessions per arm.
        #[arg(long)]
        baseline_n: usize,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}
