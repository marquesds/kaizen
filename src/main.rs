// SPDX-License-Identifier: AGPL-3.0-or-later
use clap::CommandFactory;
use clap::{Parser, Subcommand, ValueEnum};
use kaizen::DataSource;
use kaizen::feedback::types::FeedbackLabel;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

const LONG_ABOUT: &str = "Deploy and share kaizen: real-time-tailable agent sessions, retros, and experiments to improve your repo, across Cursor, Claude Code, and Codex. One SQLite store; redact before any sync. Docs: https://github.com/marquesds/kaizen/blob/main/docs/README.md";

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
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
        /// Emit JSON (same fields as the MCP `kaizen_summary` tool with json=true).
        #[arg(long)]
        json: bool,
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
        /// `local` (default) \| `provider` (remote cache) \| `mixed`. With `provider`/`mixed`, use `--refresh` to force a query pull.
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
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
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
        /// `local` \| `provider` \| `mixed` (see `kaizen summary --help`).
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
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
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
        /// Force a full agent transcript rescan (ignore `[scan].min_rescan_seconds`).
        #[arg(short, long)]
        refresh: bool,
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
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
        #[arg(long, value_enum, default_value_t = DataSource::Local)]
        source: DataSource,
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
}

#[derive(Subcommand)]
enum EvalCommand {
    /// Run LLM-as-a-Judge evals on unevaluated sessions.
    Run {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Only evaluate sessions started in the last N days.
        #[arg(long, default_value_t = 7)]
        since_days: u64,
        /// Print what would be evaluated without calling the judge.
        #[arg(long)]
        dry_run: bool,
    },
    /// List stored eval results.
    List {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
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
        /// Session ID to render the prompt for.
        session_id: String,
        /// Rubric to use.
        #[arg(long, default_value = "tool-efficiency-v1")]
        rubric: String,
    },
}

#[derive(Subcommand)]
enum PromptCommand {
    /// List all recorded prompt snapshots.
    List {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
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
    /// Append exporter template to `~/.kaizen/config.toml` (alias of `configure`).
    Init {
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Call configured provider `health`, show query settings, and exporter resolution (redacted).
    Doctor {
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Run one provider `pull` into local `remote_*` cache (stub until APIs are fully wired).
    Pull {
        /// Trailing window in days (passed to the provider; coarse).
        #[arg(long, default_value_t = 7)]
        days: u32,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Replay events from local SQLite through telemetry exporters (PostHog, Datadog, OTLP, dev).
    ///
    /// Does not POST to Kaizen ingest or modify the sync outbox. Requires `[sync].team_salt_hex`
    /// and at least one enabled `[[telemetry.exporters]]`. Re-running sends duplicates (no dedupe).
    /// Sessions pruned by `[retention].hot_days` are absent from the store (same as `retro`).
    Push {
        /// Trailing window in days (`end = now`, same idea as `retro` / `metrics`).
        #[arg(long, default_value_t = 7)]
        days: u32,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Every workspace registered for this machine (see `kaizen summary --all-workspaces`).
        #[arg(long)]
        all_workspaces: bool,
        /// Print per-workspace event and batch counts without calling exporters.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print JSON shapes for canonical telemetry items (see `sync::canonical`).
    PrintSchema,
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
    },
    /// Transition experiment from Draft to Running.
    Start {
        id: String,
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
    /// Mark experiment Archived (must be Concluded first).
    Archive {
        id: String,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Print MDE at 80% power / 95% CI for a metric given expected sample size.
    Power {
        /// tokens_per_session|cost_per_session|success_rate|…
        #[arg(long)]
        metric: String,
        /// Expected sessions per arm.
        #[arg(long)]
        baseline_n: usize,
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
        /// Read from every registered workspace on this machine.
        #[arg(long)]
        all_workspaces: bool,
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
    },
}

#[derive(Subcommand)]
enum FeedbackCommand {
    /// List feedback records for the workspace.
    List {
        #[arg(long)]
        workspace: Option<PathBuf>,
        #[arg(long, value_enum)]
        label: Option<FeedbackLabel>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        json: bool,
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
                    all_workspaces,
                    json,
                    refresh,
                },
        } => kaizen::shell::cli::cmd_sessions_list(
            workspace.as_deref(),
            json,
            refresh,
            all_workspaces,
        ),
        Command::Sessions {
            subcmd: SessionsCommand::Show { id, workspace },
        } => kaizen::shell::cli::cmd_session_show(&id, workspace.as_deref()),
        Command::Sessions {
            subcmd:
                SessionsCommand::Annotate {
                    id,
                    score,
                    label,
                    note,
                    workspace,
                },
        } => kaizen::shell::feedback::cmd_sessions_annotate(
            &id,
            score,
            label,
            note,
            workspace.as_deref(),
        ),
        Command::Sessions {
            subcmd:
                SessionsCommand::Tree {
                    id,
                    depth,
                    json,
                    workspace,
                },
        } => kaizen::shell::cli::cmd_sessions_tree(&id, depth, json, workspace.as_deref()),
        Command::Feedback {
            subcmd:
                FeedbackCommand::List {
                    workspace,
                    label,
                    since,
                    json,
                },
        } => kaizen::shell::feedback::cmd_feedback_list(workspace.as_deref(), label, since, json),
        Command::Summary {
            workspace,
            all_workspaces,
            json,
            refresh,
            source,
        } => kaizen::shell::cli::cmd_summary(
            workspace.as_deref(),
            json,
            refresh,
            all_workspaces,
            source,
        ),
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
        Command::Insights {
            workspace,
            all_workspaces,
            refresh,
            source,
        } => kaizen::shell::insights::cmd_insights(
            workspace.as_deref(),
            all_workspaces,
            refresh,
            source,
        ),
        Command::Guidance {
            days,
            json,
            workspace,
            refresh,
            source,
        } => {
            kaizen::shell::guidance::cmd_guidance(workspace.as_deref(), days, json, refresh, source)
        }
        Command::Metrics {
            subcmd,
            days,
            json,
            force,
            workspace,
            all_workspaces,
            refresh,
            source,
        } => match subcmd {
            Some(MetricsCommand::Index { workspace, force }) => {
                kaizen::shell::metrics::cmd_metrics_index(workspace.as_deref(), force)
            }
            None => kaizen::shell::metrics::cmd_metrics(
                workspace.as_deref(),
                days,
                json,
                force,
                all_workspaces,
                refresh,
                source,
            ),
        },
        Command::Sync {
            subcmd: SyncCommand::Run { workspace, once },
        } => kaizen::shell::sync::cmd_sync_run(workspace.as_deref(), once),
        Command::Sync {
            subcmd: SyncCommand::Status { workspace },
        } => kaizen::shell::sync::cmd_sync_status(workspace.as_deref()),
        Command::Telemetry { subcmd } => match subcmd {
            TelemetrySubcommand::Init { workspace } => {
                kaizen::shell::telemetry::cmd_telemetry_init(workspace.as_deref())
            }
            TelemetrySubcommand::Doctor { workspace } => {
                kaizen::shell::telemetry::cmd_telemetry_doctor(workspace.as_deref())
            }
            TelemetrySubcommand::Pull { days, workspace } => {
                kaizen::shell::telemetry::cmd_telemetry_pull(workspace.as_deref(), days)
            }
            TelemetrySubcommand::Push {
                days,
                workspace,
                all_workspaces,
                dry_run,
            } => kaizen::shell::telemetry::cmd_telemetry_push(
                workspace.as_deref(),
                all_workspaces,
                days,
                dry_run,
            ),
            TelemetrySubcommand::PrintSchema => {
                kaizen::shell::telemetry::cmd_telemetry_print_schema()
            }
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
            source,
        } => kaizen::shell::retro::cmd_retro(
            workspace.as_deref(),
            days,
            dry_run,
            json,
            force,
            refresh,
            source,
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
        Command::Eval { subcmd } => match subcmd {
            EvalCommand::Run {
                workspace,
                since_days,
                dry_run,
            } => kaizen::shell::eval::cmd_eval_run(workspace.as_deref(), since_days, dry_run),
            EvalCommand::List {
                workspace,
                min_score,
                json,
            } => kaizen::shell::eval::cmd_eval_list(workspace.as_deref(), min_score, json),
            EvalCommand::Prompt {
                workspace,
                session_id,
                rubric,
            } => kaizen::shell::eval::cmd_eval_prompt(workspace.as_deref(), &session_id, &rubric),
        },
        Command::Prompt { subcmd } => match subcmd {
            PromptCommand::List { workspace, json } => {
                kaizen::shell::prompt::cmd_prompt_list(workspace.as_deref(), json)
            }
            PromptCommand::Show {
                fingerprint,
                workspace,
                json,
            } => kaizen::shell::prompt::cmd_prompt_show(&fingerprint, workspace.as_deref(), json),
            PromptCommand::Diff {
                fingerprint_a,
                fingerprint_b,
                workspace,
            } => kaizen::shell::prompt::cmd_prompt_diff(
                &fingerprint_a,
                &fingerprint_b,
                workspace.as_deref(),
            ),
        },
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
            control_branch,
            treatment_branch,
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
                control_branch,
                treatment_branch,
            },
        ),
        ExpCommand::Start { id, workspace } => exp::cmd_start(workspace.as_deref(), &id),
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
        ExpCommand::Archive { id, workspace } => exp::cmd_archive(workspace.as_deref(), &id),
        ExpCommand::Power {
            metric,
            baseline_n,
            workspace,
        } => exp::cmd_power(workspace.as_deref(), &metric, baseline_n),
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
