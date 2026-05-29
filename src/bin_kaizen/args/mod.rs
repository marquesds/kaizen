// SPDX-License-Identifier: AGPL-3.0-or-later
use clap::{Parser, Subcommand, ValueEnum};
use kaizen::DataSource;
use kaizen::feedback::types::FeedbackLabel;
use std::path::PathBuf;

const LONG_ABOUT: &str = "Deploy and share kaizen: real-time-tailable agent sessions, retros, and experiments to improve your repo, across Cursor, Claude Code, Codex, and Mistral Vibe. One SQLite store; redact before any sync. Docs: https://github.com/marquesds/kaizen/blob/main/docs/README.md";

mod experiment;
mod improve;
mod interchange;
mod operate;
mod shared;
mod telemetry;
mod trust;

pub(crate) use experiment::*;
pub(crate) use improve::*;
pub(crate) use interchange::*;
pub(crate) use operate::*;
pub(crate) use shared::*;
pub(crate) use telemetry::*;
pub(crate) use trust::*;

#[derive(Parser)]
#[command(
    name = "kaizen",
    about = "AI agent session telemetry and insights",
    long_about = LONG_ABOUT,
    version,
    propagate_version = true
)]
pub(crate) struct Cli {
    /// Keep Phase 0-2 direct SQLite mode for this invocation.
    #[arg(long, global = true)]
    pub(crate) no_daemon: bool,
    #[command(subcommand)]
    pub(crate) cmd: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Ingest events from hooks or other sources.
    #[command(next_help_heading = "Operate")]
    Ingest {
        #[command(subcommand)]
        subcmd: IngestCommand,
    },
    /// Manage the local Kaizen daemon.
    #[command(next_help_heading = "Operate")]
    Daemon {
        #[command(subcommand)]
        subcmd: DaemonCommand,
    },
    /// Session list/show commands.
    #[command(next_help_heading = "Trust & observe")]
    Sessions {
        #[command(subcommand)]
        subcmd: SessionsCommand,
    },
    /// Search indexed session events.
    #[command(next_help_heading = "Trust & observe")]
    Search {
        #[command(subcommand)]
        subcmd: SearchCommand,
    },
    /// Structured trace query over local session events.
    #[command(next_help_heading = "Trust & observe")]
    Query {
        expr: String,
        #[arg(long)]
        since: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Aggregate session + cost stats across all agents.
    #[command(next_help_heading = "Trust & observe")]
    Summary {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
        /// Emit JSON (same fields as the MCP `kaizen_summary` tool with json=true).
        #[arg(long)]
        json: bool,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
        /// `local` (default) | `provider` (remote cache) | `mixed`. With `provider`/`mixed`, `--refresh` can call remote APIs.
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
    },
    /// Open interactive TUI.
    #[command(next_help_heading = "Trust & observe")]
    Tui {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Idempotent workspace setup (writes config, patches hooks, installs skill).
    #[command(next_help_heading = "Trust & observe")]
    Init {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Start proxy tasks and report deep model-call capture readiness.
        #[arg(long)]
        deep: bool,
    },
    /// Verify config, store, and hook wiring for this workspace.
    #[command(next_help_heading = "Trust & observe")]
    Doctor {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Load previous local agent sessions into Kaizen stores.
    #[command(next_help_heading = "Trust & observe")]
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
    /// Prune local sessions older than retention window (see `[retention].hot_days` or `--days`).
    #[command(next_help_heading = "Operate")]
    Gc {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Keep sessions started within the last N days (overrides config when set).
        #[arg(long)]
        days: Option<u32>,
        /// Run VACUUM after delete (slow; reclaims file space).
        #[arg(long)]
        vacuum: bool,
    },
    /// Migrate local store between SQLite-only and tiered storage.
    #[command(next_help_heading = "Operate")]
    Migrate {
        #[command(subcommand)]
        subcmd: MigrateCommand,
    },
    /// Rich session insights: activity by day, top tools, recent sessions.
    #[command(next_help_heading = "Trust & observe")]
    Insights {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
        /// `local` | `provider` | `mixed`; `--refresh` can call remote APIs.
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
    },
    /// Skill and Cursor rule adoption from observed path refs in payloads (not silent injection).
    #[command(next_help_heading = "Trust & observe")]
    Guidance {
        #[command(subcommand)]
        subcmd: Option<GuidanceCommand>,
        /// Trailing window in days (default 7).
        #[arg(long, default_value_t = 7)]
        days: u32,
        /// Emit JSON report.
        #[arg(long)]
        json: bool,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
        /// `local` | `provider` | `mixed`; `--refresh` can call remote APIs.
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
    },
    /// Smart metrics: code hotspots, slow tools, token sinks.
    #[command(next_help_heading = "Trust & observe")]
    Metrics {
        #[command(subcommand)]
        subcmd: Option<MetricsCommand>,
        /// Trailing window in days (default 7).
        #[arg(long, default_value_t = 7)]
        days: u32,
        /// Emit JSON report.
        #[arg(long)]
        json: bool,
        /// Rebuild repo snapshot even if fingerprint unchanged.
        #[arg(long)]
        force: bool,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
        /// `local` | `provider` | `mixed`; `--refresh` can call remote APIs.
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
    },
    /// Run an agent command with Kaizen proxy/session env.
    #[command(next_help_heading = "Trust & observe", trailing_var_arg = true)]
    Observe {
        /// Agent profile: claude, codex, cursor, or auto.
        #[arg(long, default_value = "auto")]
        agent: String,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Command and arguments to run.
        #[arg(required = true, num_args = 1.., allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Flush local outbox to the configured ingest endpoint.
    #[command(next_help_heading = "Operate")]
    Sync {
        #[command(subcommand)]
        subcmd: SyncCommand,
    },
    /// Optional telemetry sinks (file NDJSON, PostHog, Datadog, OTLP, dev) alongside Kaizen sync.
    #[command(next_help_heading = "Operate")]
    Telemetry {
        #[command(subcommand)]
        subcmd: TelemetrySubcommand,
    },
    /// Export local telemetry into interchange formats.
    #[command(next_help_heading = "Integrations")]
    Export {
        #[command(subcommand)]
        subcmd: ExportCommand,
    },
    /// Import local telemetry from interchange formats.
    #[command(next_help_heading = "Integrations")]
    Import {
        #[command(subcommand)]
        subcmd: ImportCommand,
    },
    /// Verify local audit invariants.
    #[command(next_help_heading = "Integrations")]
    Verify {
        #[command(subcommand)]
        subcmd: VerifyCommand,
    },
    /// Experiment binding + report.
    #[command(next_help_heading = "Improve")]
    Exp {
        #[command(subcommand)]
        subcmd: ExpCommand,
    },
    /// Mine and manage local regression cases.
    #[command(next_help_heading = "Improve")]
    Cases {
        #[command(subcommand)]
        subcmd: CasesCommand,
    },
    /// Local automation rules over trace queries.
    #[command(next_help_heading = "Improve")]
    Rules {
        #[command(subcommand)]
        subcmd: RulesCommand,
    },
    /// Built-in local health alerts.
    #[command(next_help_heading = "Improve")]
    Alerts {
        #[command(subcommand)]
        subcmd: AlertsCommand,
    },
    /// Local review queue from rules and cases.
    #[command(next_help_heading = "Improve")]
    Review {
        #[command(subcommand)]
        subcmd: ReviewCommand,
    },
    /// Weekly-style heuristic retro report.
    #[command(next_help_heading = "Improve")]
    Retro {
        /// Trailing window in days (default 7).
        #[arg(long, default_value_t = 7)]
        days: u32,
        /// Print Markdown to stdout; do not write a file.
        #[arg(long)]
        dry_run: bool,
        /// Emit JSON report on stdout (no file write).
        #[arg(long)]
        json: bool,
        /// Overwrite this ISO week's report if it exists.
        #[arg(long)]
        force: bool,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Force a full agent transcript rescan before reading. This can take a while on large workspaces.
        #[arg(short, long)]
        refresh: bool,
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
    },
    /// List registered workspaces on this machine.
    #[command(next_help_heading = "Trust & observe")]
    Projects {
        #[command(subcommand)]
        subcmd: ProjectsCommand,
    },
    /// Model Context Protocol server (stdio) — see docs/mcp.md.
    #[command(next_help_heading = "Integrations")]
    Mcp,
    /// Upgrade kaizen to the latest release.
    #[command(next_help_heading = "Operate")]
    Upgrade {
        /// Build from crates.io instead of installing a release binary.
        #[arg(long)]
        from_source: bool,
    },
    /// Print shell completion script to stdout; redirect or eval to install.
    #[command(next_help_heading = "Shell")]
    Completions {
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    /// Local HTTP forwarder for Anthropic-style APIs + proxy telemetry. See docs/llm-proxy.md.
    #[command(next_help_heading = "Operate")]
    Proxy {
        #[command(subcommand)]
        subcmd: ProxyCommand,
    },
    /// LLM-as-a-Judge evaluations for agent sessions. See docs/usage.md.
    #[command(next_help_heading = "Improve")]
    Eval {
        #[command(subcommand)]
        subcmd: EvalCommand,
    },
    /// Prompt/system-prompt version tracking. See docs/usage.md.
    #[command(next_help_heading = "Improve")]
    Prompt {
        #[command(subcommand)]
        subcmd: PromptCommand,
    },
    /// Human feedback on agent sessions (score/label/note).
    #[command(next_help_heading = "Improve")]
    Feedback {
        #[command(subcommand)]
        subcmd: FeedbackCommand,
    },
    /// Post-stop test/lint outcomes (opt-in). See docs/outcomes.md.
    #[command(next_help_heading = "Trust & observe")]
    Outcomes {
        #[command(subcommand)]
        subcmd: OutcomesCommand,
    },
    /// Internal: sample OS stats for a hook PID. Spawned by ingest when opt-in.
    #[command(hide = true, name = "__sampler-run")]
    SamplerRun {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        session: String,
        #[arg(long)]
        pid: u32,
    },
}

#[cfg(test)]
mod cli_parser_tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn clap_cli_debug_assert() {
        Cli::command().debug_assert();
    }
}
