use super::*;

#[derive(Subcommand)]
pub(crate) enum EvalCommand {
    /// Run LLM-as-a-Judge evals on unevaluated sessions.
    Run {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Only evaluate sessions started in the last N days.
        #[arg(long, default_value_t = 7)]
        since_days: u64,
        /// Print what would be evaluated without calling the judge.
        #[arg(long)]
        dry_run: bool,
        /// Emit JSON array.
        #[arg(long)]
        json: bool,
    },
    /// List stored eval results.
    List {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Only show sessions with score >= this value (0.0 = show all).
        #[arg(long, default_value_t = 0.0)]
        min_score: f64,
        /// Emit JSON array.
        #[arg(long)]
        json: bool,
    },
    /// Print the rendered judge prompt for a session (no LLM call).
    Prompt {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Session ID to render the prompt for.
        session_id: String,
        /// Rubric to use.
        #[arg(long, default_value = "tool-efficiency-v1")]
        rubric: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum CasesCommand {
    /// Mine cases from low evals and bad feedback.
    Mine(SharedSinceJson),
    /// Create one case for a session.
    Create {
        #[arg(long)]
        session: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        ws: WorkspaceFlags,
    },
    /// List cases.
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        ws: WorkspaceFlags,
    },
    /// Show one case.
    Show(IdJson),
    /// Archive one case.
    Archive(IdOnly),
}

#[derive(Subcommand)]
pub(crate) enum RulesCommand {
    /// Create local rule.
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        filter: String,
        #[arg(long)]
        action: String,
        #[arg(long)]
        message: Option<String>,
        #[command(flatten)]
        ws: WorkspaceFlags,
    },
    /// List rules.
    List(JsonOnly),
    /// Run enabled rules.
    Run {
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        ws: WorkspaceFlags,
    },
    /// Enable a rule.
    Enable(IdOnly),
    /// Disable a rule.
    Disable(IdOnly),
}

#[derive(Subcommand)]
pub(crate) enum AlertsCommand {
    /// Check built-in alert conditions.
    Check {
        #[arg(long, default_value_t = 7)]
        days: u64,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        ws: WorkspaceFlags,
    },
}

#[derive(Subcommand)]
pub(crate) enum ReviewCommand {
    /// List review items.
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        ws: WorkspaceFlags,
    },
    /// Show one review item.
    Show(IdJson),
    /// Mark review item resolved.
    Resolve(IdOnly),
    /// Mark review item dismissed.
    Dismiss(IdOnly),
}
