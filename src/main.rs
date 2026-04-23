// SPDX-License-Identifier: AGPL-3.0-or-later
use clap::{Parser, Subcommand, ValueEnum};
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "kaizen", about = "Agent session tracker")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Ingest events from hooks or other sources.
    Ingest {
        #[command(subcommand)]
        subcmd: IngestCommand,
    },
    /// Session list/show commands.
    Sessions {
        #[command(subcommand)]
        subcmd: SessionsCommand,
    },
    /// Aggregate session + cost stats across all agents.
    Summary {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Open interactive TUI.
    Tui {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Idempotent workspace setup (writes config, patches hooks, installs skill).
    Init {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Rich session insights: activity by day, top tools, recent sessions.
    Insights {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Smart metrics: code hotspots, slow tools, token sinks.
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
    },
    /// Flush local outbox to the configured ingest endpoint.
    Sync {
        #[command(subcommand)]
        subcmd: SyncCommand,
    },
    /// Optional third-party telemetry sinks (PostHog, Datadog, OTLP, dev) alongside Kaizen sync.
    Telemetry {
        #[command(subcommand)]
        subcmd: TelemetrySubcommand,
    },
    /// Experiment binding + report.
    Exp {
        #[command(subcommand)]
        subcmd: ExpCommand,
    },
    /// Weekly-style heuristic retro report.
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
    },
    /// Model Context Protocol server (stdio) — see docs/mcp.md.
    Mcp,
}

#[derive(Subcommand)]
enum TelemetrySubcommand {
    /// Append `[[telemetry.exporters]]` template to `~/.kaizen/config.toml` (editor fill-in).
    Configure {
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Redacted: merged telemetry exporter resolution (TOML + env).
    PrintEffectiveConfig {
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ExpCommand {
    /// Create experiment (records control/treatment commits).
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
        /// git|manual
        #[arg(long, default_value = "git")]
        bind: String,
        #[arg(long, default_value_t = 14)]
        duration_days: u32,
        /// target delta pct, e.g. -10.0 for -10%
        #[arg(long, default_value_t = -10.0)]
        target_pct: f64,
        #[arg(long)]
        control_commit: Option<String>,
        #[arg(long)]
        treatment_commit: Option<String>,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// List all experiments.
    List {
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Show one experiment's metadata.
    Status {
        id: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
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
    },
    /// Render markdown (or JSON) report with bootstrap CI.
    Report {
        id: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Mark experiment Concluded.
    Conclude {
        id: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SyncCommand {
    /// Run sync loop until interrupted (or use --once).
    Run {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Single flush then exit (for tests / scripts).
        #[arg(long)]
        once: bool,
    },
    /// Show outbox depth and last flush / error state.
    Status {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum MetricsCommand {
    /// Rebuild repo snapshot and Ladybug sidecar.
    Index {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Rebuild even when fingerprint unchanged.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum IngestCommand {
    /// Read hook event from stdin and log it.
    Hook {
        /// hook source agent
        #[arg(long, value_enum)]
        source: Source,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SessionsCommand {
    /// List sessions for current workspace.
    List {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Show full details for a session.
    Show {
        id: String,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum Source {
    Cursor,
    Claude,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.cmd {
        Command::Ingest {
            subcmd: IngestCommand::Hook { source, workspace },
        } => ingest_hook(source, workspace),
        Command::Sessions {
            subcmd: SessionsCommand::List { workspace },
        } => kaizen::shell::cli::cmd_sessions_list(workspace.as_deref()),
        Command::Sessions {
            subcmd: SessionsCommand::Show { id, workspace },
        } => kaizen::shell::cli::cmd_session_show(&id, workspace.as_deref()),
        Command::Summary { workspace } => kaizen::shell::cli::cmd_summary(workspace.as_deref()),
        Command::Tui { workspace } => tokio::runtime::Runtime::new()?.block_on(
            kaizen::ui::tui::run(workspace.as_deref().unwrap_or(&std::env::current_dir()?)),
        ),
        Command::Init { workspace } => kaizen::shell::cli::cmd_init(workspace.as_deref()),
        Command::Insights { workspace } => kaizen::shell::cli::cmd_insights(workspace.as_deref()),
        Command::Metrics {
            subcmd,
            days,
            json,
            force,
            workspace,
        } => match subcmd {
            Some(MetricsCommand::Index { workspace, force }) => {
                kaizen::shell::metrics::cmd_metrics_index(workspace.as_deref(), force)
            }
            None => kaizen::shell::metrics::cmd_metrics(workspace.as_deref(), days, json, force),
        },
        Command::Sync {
            subcmd: SyncCommand::Run { workspace, once },
        } => kaizen::shell::sync::cmd_sync_run(workspace.as_deref(), once),
        Command::Sync {
            subcmd: SyncCommand::Status { workspace },
        } => kaizen::shell::sync::cmd_sync_status(workspace.as_deref()),
        Command::Telemetry { subcmd } => match subcmd {
            TelemetrySubcommand::Configure { workspace } => {
                kaizen::shell::telemetry::cmd_telemetry_configure(workspace.as_deref())
            }
            TelemetrySubcommand::PrintEffectiveConfig { workspace } => {
                kaizen::shell::telemetry::cmd_telemetry_print_effective(workspace.as_deref())
            }
        },
        Command::Retro {
            days,
            dry_run,
            json,
            force,
            workspace,
        } => kaizen::shell::retro::cmd_retro(workspace.as_deref(), days, dry_run, json, force),
        Command::Exp { subcmd } => dispatch_exp(subcmd),
        Command::Mcp => {
            // Requires multi-threaded runtime (rmcp + spawn_blocking in tools)
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(kaizen::mcp::run_stdio_server())
        }
    }
}

fn dispatch_exp(cmd: ExpCommand) -> anyhow::Result<()> {
    use kaizen::shell::exp;
    match cmd {
        ExpCommand::New {
            name,
            hypothesis,
            change,
            metric,
            bind,
            duration_days,
            target_pct,
            control_commit,
            treatment_commit,
            workspace,
        } => exp::cmd_new(
            workspace.as_deref(),
            exp::NewArgs {
                name,
                hypothesis,
                change,
                metric,
                bind,
                duration_days,
                target_pct,
                control_commit,
                treatment_commit,
            },
        ),
        ExpCommand::List { workspace } => exp::cmd_list(workspace.as_deref()),
        ExpCommand::Status { id, workspace } => exp::cmd_status(workspace.as_deref(), &id),
        ExpCommand::Tag {
            id,
            session,
            variant,
            workspace,
        } => exp::cmd_tag(workspace.as_deref(), &id, &session, &variant),
        ExpCommand::Report {
            id,
            json,
            workspace,
        } => exp::cmd_report(workspace.as_deref(), &id, json),
        ExpCommand::Conclude { id, workspace } => exp::cmd_conclude(workspace.as_deref(), &id),
    }
}

fn ingest_hook(source: Source, workspace: Option<PathBuf>) -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let src = match source {
        Source::Cursor => kaizen::shell::ingest::IngestSource::Cursor,
        Source::Claude => kaizen::shell::ingest::IngestSource::Claude,
    };
    kaizen::shell::ingest::ingest_hook_text(src, &input, workspace)
}
