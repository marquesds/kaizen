// SPDX-License-Identifier: AGPL-3.0-or-later
use clap::CommandFactory;
use clap::{Parser, Subcommand, ValueEnum};
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

const LONG_ABOUT: &str = "Local-first telemetry for AI coding agents. Collect, query, and improve how agents use your repo — offline by default. Docs: https://github.com/lucasmarqs/kaizen/blob/main/docs/README.md";

#[derive(Parser)]
#[command(
    name = "kaizen",
    about = "AI agent session telemetry and insights",
    long_about = LONG_ABOUT,
    version,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Ingest events from hooks or other sources.
    #[command(next_help_heading = "Operate")]
    Ingest {
        #[command(subcommand)]
        subcmd: IngestCommand,
    },
    /// Session list/show commands.
    #[command(next_help_heading = "Trust & observe")]
    Sessions {
        #[command(subcommand)]
        subcmd: SessionsCommand,
    },
    /// Aggregate session + cost stats across all agents.
    #[command(next_help_heading = "Trust & observe")]
    Summary {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Emit JSON (same fields as the MCP `kaizen_summary` tool with json=true).
        #[arg(long)]
        json: bool,
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
    },
    /// Open interactive TUI.
    #[command(next_help_heading = "Trust & observe")]
    Tui {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Idempotent workspace setup (writes config, patches hooks, installs skill).
    #[command(next_help_heading = "Trust & observe")]
    Init {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Verify config, store, and hook wiring for this workspace.
    #[command(next_help_heading = "Trust & observe")]
    Doctor {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Prune local sessions older than retention window (see `[retention].hot_days` or `--days`).
    #[command(next_help_heading = "Operate")]
    Gc {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Keep sessions started within the last N days (overrides config when set).
        #[arg(long)]
        days: Option<u32>,
        /// Run VACUUM after delete (slow; reclaims file space).
        #[arg(long)]
        vacuum: bool,
    },
    /// Rich session insights: activity by day, top tools, recent sessions.
    #[command(next_help_heading = "Trust & observe")]
    Insights {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
    },
    /// Skill and Cursor rule adoption from observed path refs in payloads (not silent injection).
    #[command(next_help_heading = "Trust & observe")]
    Guidance {
        /// Trailing window in days (default 7).
        #[arg(long, default_value_t = 7)]
        days: u32,
        /// Emit JSON report.
        #[arg(long)]
        json: bool,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
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
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
    },
    /// Flush local outbox to the configured ingest endpoint.
    #[command(next_help_heading = "Operate")]
    Sync {
        #[command(subcommand)]
        subcmd: SyncCommand,
    },
    /// Optional third-party telemetry sinks (PostHog, Datadog, OTLP, dev) alongside Kaizen sync.
    #[command(next_help_heading = "Operate")]
    Telemetry {
        #[command(subcommand)]
        subcmd: TelemetrySubcommand,
    },
    /// Experiment binding + report.
    #[command(next_help_heading = "Improve")]
    Exp {
        #[command(subcommand)]
        subcmd: ExpCommand,
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
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
    },
    /// Model Context Protocol server (stdio) — see docs/mcp.md.
    #[command(next_help_heading = "Integrations")]
    Mcp,
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
}

/// Shells supported by clap_complete (redirect stdout to a file, or eval).
#[derive(Copy, Clone, Debug, ValueEnum, Eq, PartialEq)]
enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    Powershell,
    Zsh,
}

#[derive(Subcommand)]
enum ProxyCommand {
    /// Bind and forward until interrupted.
    Run {
        /// Address to listen, e.g. 127.0.0.1:3847 (overrides [proxy] in config TOML).
        #[arg(long)]
        listen: Option<String>,
        /// Upstream base URL, e.g. https://api.anthropic.com (no trailing slash).
        #[arg(long)]
        upstream: Option<String>,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
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
        /// Emit JSON (same as MCP with json=true)
        #[arg(long)]
        json: bool,
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
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
            subcmd:
                SessionsCommand::List {
                    workspace,
                    json,
                    refresh,
                },
        } => kaizen::shell::cli::cmd_sessions_list(workspace.as_deref(), json, refresh),
        Command::Sessions {
            subcmd: SessionsCommand::Show { id, workspace },
        } => kaizen::shell::cli::cmd_session_show(&id, workspace.as_deref()),
        Command::Summary {
            workspace,
            json,
            refresh,
        } => kaizen::shell::cli::cmd_summary(workspace.as_deref(), json, refresh),
        Command::Tui { workspace } => tokio::runtime::Runtime::new()?.block_on(
            kaizen::ui::tui::run(workspace.as_deref().unwrap_or(&std::env::current_dir()?)),
        ),
        Command::Init { workspace } => kaizen::shell::cli::cmd_init(workspace.as_deref()),
        Command::Doctor { workspace } => {
            let code = kaizen::shell::doctor::cmd_doctor(workspace.as_deref())?;
            // Non-zero: store/IO failure; hooks missing stay 0
            if code != 0 {
                std::process::exit(code);
            }
            Ok(())
        }
        Command::Gc {
            workspace,
            days,
            vacuum,
        } => kaizen::shell::gc::cmd_gc(workspace.as_deref(), days, vacuum),
        Command::Completions { shell } => {
            let sh = match shell {
                CompletionShell::Bash => clap_complete::Shell::Bash,
                CompletionShell::Elvish => clap_complete::Shell::Elvish,
                CompletionShell::Fish => clap_complete::Shell::Fish,
                CompletionShell::Powershell => clap_complete::Shell::PowerShell,
                CompletionShell::Zsh => clap_complete::Shell::Zsh,
            };
            let mut cmd = Cli::command();
            clap_complete::generate(sh, &mut cmd, "kaizen", &mut std::io::stdout());
            let _ = std::io::stdout().flush();
            Ok(())
        }
        Command::Insights { workspace, refresh } => {
            kaizen::shell::cli::cmd_insights(workspace.as_deref(), refresh)
        }
        Command::Guidance {
            days,
            json,
            workspace,
            refresh,
        } => kaizen::shell::guidance::cmd_guidance(workspace.as_deref(), days, json, refresh),
        Command::Metrics {
            subcmd,
            days,
            json,
            force,
            workspace,
            refresh,
        } => match subcmd {
            Some(MetricsCommand::Index { workspace, force }) => {
                kaizen::shell::metrics::cmd_metrics_index(workspace.as_deref(), force)
            }
            None => kaizen::shell::metrics::cmd_metrics(
                workspace.as_deref(),
                days,
                json,
                force,
                refresh,
            ),
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
            refresh,
        } => kaizen::shell::retro::cmd_retro(
            workspace.as_deref(),
            days,
            dry_run,
            json,
            force,
            refresh,
        ),
        Command::Exp { subcmd } => dispatch_exp(subcmd),
        Command::Mcp => {
            // Requires multi-threaded runtime (rmcp + spawn_blocking in tools)
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(kaizen::mcp::run_stdio_server())
        }
        Command::Proxy {
            subcmd:
                ProxyCommand::Run {
                    listen,
                    upstream,
                    workspace,
                },
        } => kaizen::shell::proxy::cmd_proxy_run(workspace.as_deref(), listen, upstream),
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
