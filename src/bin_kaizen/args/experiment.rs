use super::*;

#[derive(clap::Args)]
pub(crate) struct ExpNewCommand {
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) hypothesis: String,
    #[arg(long)]
    pub(crate) change: String,
    /// tokens_per_session|cost_per_session|success_rate|tool_loops|duration_minutes|files_per_session
    #[arg(long)]
    pub(crate) metric: String,
    /// git|branch|manual
    #[arg(long, default_value = "git")]
    pub(crate) bind: String,
    #[arg(long, default_value_t = 14)]
    pub(crate) duration_days: u32,
    /// target delta pct, e.g. -10.0 for -10%
    #[arg(long, default_value_t = -10.0, allow_hyphen_values = true)]
    pub(crate) target_pct: f64,
    #[arg(long)]
    pub(crate) control_commit: Option<String>,
    #[arg(long)]
    pub(crate) treatment_commit: Option<String>,
    #[arg(long)]
    pub(crate) control_branch: Option<String>,
    #[arg(long)]
    pub(crate) treatment_branch: Option<String>,
    #[arg(long)]
    pub(crate) control_fingerprint: Option<String>,
    #[arg(long)]
    pub(crate) treatment_fingerprint: Option<String>,
    #[arg(long)]
    pub(crate) workspace: Option<PathBuf>,
    /// project name shorthand for --workspace (mutually exclusive)
    #[arg(long, conflicts_with = "workspace")]
    pub(crate) project: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum ExpCommand {
    /// Create experiment in Draft state (records control/treatment commits).
    New(Box<ExpNewCommand>),
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
