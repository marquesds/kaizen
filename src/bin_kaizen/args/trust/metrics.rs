use clap::Args;
use kaizen::DataSource;
use std::path::PathBuf;

#[derive(Args)]
pub(crate) struct MetricsReportArgs {
    /// Trailing window in days (default 7).
    #[arg(long, default_value_t = 7)]
    pub(crate) days: u32,
    /// Emit JSON report.
    #[arg(long)]
    pub(crate) json: bool,
    /// Rebuild repo snapshot even if fingerprint unchanged.
    #[arg(long)]
    pub(crate) force: bool,
    /// workspace root (default: cwd)
    #[arg(long)]
    pub(crate) workspace: Option<PathBuf>,
    /// project name shorthand for --workspace (mutually exclusive)
    #[arg(long, conflicts_with = "workspace")]
    pub(crate) project: Option<String>,
    /// Read from every registered workspace on this machine.
    #[arg(long)]
    pub(crate) all_workspaces: bool,
    /// Force a full agent transcript rescan before reading.
    #[arg(short, long)]
    pub(crate) refresh: bool,
    /// `local` | `provider` | `mixed`; `--refresh` can call remote APIs.
    #[arg(long, value_enum, default_value_t = DataSource::Local)]
    pub(crate) source: DataSource,
}
