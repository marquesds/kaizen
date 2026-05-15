// SPDX-License-Identifier: AGPL-3.0-or-later
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use kaizen::DataSource;
use kaizen::feedback::types::FeedbackLabel;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

const LONG_ABOUT: &str = "Deploy and share kaizen: real-time-tailable agent sessions, retros, and experiments to improve your repo, across Cursor, Claude Code, Codex, and Mistral Vibe. One SQLite store; redact before any sync. Docs: https://github.com/marquesds/kaizen/blob/main/docs/README.md";

#[derive(Parser)]
#[command(
    name = "kaizen",
    about = "AI agent session telemetry and insights",
    long_about = LONG_ABOUT,
    version,
    propagate_version = true
)]
struct Cli {
    /// Keep Phase 0-2 direct SQLite mode for this invocation.
    #[arg(long, global = true)]
    no_daemon: bool,
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

#[derive(Subcommand)]
enum EvalCommand {
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
enum ProjectsCommand {
    /// List registered workspaces.
    List,
}

#[derive(Subcommand)]
enum PromptCommand {
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
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum TelemetrySubcommand {
    /// Append exporter template to `~/.kaizen/config.toml` (alias of `configure`).
    Init {
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Exporter template to append without prompting.
        #[arg(long = "type", value_enum)]
        exporter_type: Option<TelemetryExporterKind>,
        /// File exporter path, absolute or relative to each workspace.
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Call configured provider `health`, show query settings, and exporter resolution (redacted).
    Doctor {
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Run one provider `pull` into local `remote_*` cache (stub until APIs are fully wired).
    Pull {
        /// Trailing window in days (passed to the provider; coarse).
        #[arg(long, default_value_t = 7)]
        days: u32,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
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
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Every workspace registered for this machine (see `kaizen summary --all-workspaces`).
        #[arg(long)]
        all_workspaces: bool,
        /// Print per-workspace event and batch counts without calling exporters.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print JSON shapes for canonical telemetry items (see `sync::canonical`).
    PrintSchema,
    /// Validating wizard: append a `[[telemetry.exporters]]` row after live `health` succeeds.
    Configure {
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// Exporter template to append without prompting.
        #[arg(long = "type", value_enum)]
        exporter_type: Option<TelemetryExporterKind>,
        /// File exporter path, absolute or relative to each workspace.
        #[arg(long)]
        path: Option<PathBuf>,
        /// API key (DD_API_KEY for datadog, POSTHOG_API_KEY for posthog). Falls back to env.
        #[arg(long)]
        api_key: Option<String>,
        /// Datadog site (e.g. `datadoghq.com`, `us5.datadoghq.com`). Falls back to DD_SITE.
        #[arg(long)]
        site: Option<String>,
        /// PostHog host. Falls back to POSTHOG_HOST.
        #[arg(long)]
        host: Option<String>,
        /// OTLP endpoint. Falls back to OTEL_EXPORTER_OTLP_ENDPOINT.
        #[arg(long)]
        endpoint: Option<String>,
        /// Fail instead of prompting for missing values; for scripts and CI.
        #[arg(long)]
        non_interactive: bool,
    },
    /// Send one synthetic event to every configured `[[telemetry.exporters]]` and report ok/fail.
    Test {
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Redacted: merged telemetry exporter resolution (TOML + env).
    PrintEffectiveConfig {
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
    /// Read local NDJSON from the `file` exporter (default: `<workspace>/.kaizen/telemetry.ndjson`).
    Tail {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// File path (absolute or relative to workspace).
        #[arg(long, short = 'f')]
        file: Option<PathBuf>,
        /// Print current file contents and exit (no follow).
        #[arg(long)]
        no_follow: bool,
        /// Pretty-print each JSON line.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum TelemetryExporterKind {
    File,
    Posthog,
    Datadog,
    Otlp,
    Dev,
}

impl TelemetryExporterKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Posthog => "posthog",
            Self::Datadog => "datadog",
            Self::Otlp => "otlp",
            Self::Dev => "dev",
        }
    }
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

#[derive(Subcommand)]
enum SyncCommand {
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
enum DaemonCommand {
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
enum MetricsCommand {
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
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum SessionsCommand {
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
enum SearchCommand {
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
enum OutcomesCommand {
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
enum FeedbackCommand {
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

#[derive(ValueEnum, Clone, Debug)]
enum Source {
    Cursor,
    Claude,
    Vibe,
}

#[derive(Subcommand)]
enum MigrateCommand {
    /// Export SQLite rows into hot log + cold Parquet.
    V2 {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// Keep future/skewed timestamps instead of failing validation.
        #[arg(long)]
        allow_skew: bool,
    },
    /// Restore raw SQLite events from hot log + cold Parquet.
    V1 {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

fn resolve_ws(
    workspace: Option<&std::path::Path>,
    project: Option<&str>,
) -> anyhow::Result<Option<PathBuf>> {
    match (workspace, project) {
        (None, None) => Ok(None),
        (w, p) => kaizen::shell::cli::resolve_target(w, p).map(|(path, _)| Some(path)),
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    if cli.no_daemon {
        unsafe { std::env::set_var("KAIZEN_DAEMON", "0") };
    }
    match cli.cmd {
        Command::Daemon { subcmd } => dispatch_daemon(subcmd),
        Command::Ingest {
            subcmd:
                IngestCommand::Hook {
                    source,
                    workspace,
                    project,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            ingest_hook(source, ws)
        }
        Command::Sessions {
            subcmd:
                SessionsCommand::List {
                    workspace,
                    project,
                    all_workspaces,
                    json,
                    limit,
                    refresh,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_sessions_list(
                ws.as_deref(),
                json,
                refresh,
                all_workspaces,
                limit,
            )
        }
        Command::Sessions {
            subcmd:
                SessionsCommand::Show {
                    id,
                    workspace,
                    project,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_session_show(&id, ws.as_deref())
        }
        Command::Sessions {
            subcmd:
                SessionsCommand::Annotate {
                    id,
                    score,
                    label,
                    note,
                    workspace,
                    project,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::feedback::cmd_sessions_annotate(&id, score, label, note, ws.as_deref())
        }
        Command::Sessions {
            subcmd:
                SessionsCommand::Tree {
                    id,
                    depth,
                    json,
                    workspace,
                    project,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_sessions_tree(&id, depth, json, ws.as_deref())
        }
        Command::Sessions {
            subcmd:
                SessionsCommand::Search {
                    query,
                    since,
                    agent,
                    kind,
                    limit,
                    workspace,
                    project,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::search::cmd_sessions_search(
                ws.as_deref(),
                &query,
                since.as_deref(),
                agent.as_deref(),
                kind.as_deref(),
                limit,
            )
        }
        Command::Search {
            subcmd: SearchCommand::Reindex { workspace, project },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::search::cmd_search_reindex(ws.as_deref())
        }
        Command::Feedback {
            subcmd:
                FeedbackCommand::List {
                    workspace,
                    project,
                    label,
                    since,
                    json,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::feedback::cmd_feedback_list(ws.as_deref(), label, since, json)
        }
        Command::Summary {
            workspace,
            project,
            all_workspaces,
            json,
            refresh,
            source,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_summary(ws.as_deref(), json, refresh, all_workspaces, source)
        }
        Command::Tui { workspace, project } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?
                .map(Ok)
                .unwrap_or_else(|| kaizen::core::workspace::resolve(None))?;
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            let result = rt.block_on(kaizen::ui::tui::run(&ws));
            rt.shutdown_timeout(std::time::Duration::from_millis(500));
            result
        }
        Command::Init { workspace, project } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_init(ws.as_deref())
        }
        Command::Doctor { workspace, project } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            let code = kaizen::shell::doctor::cmd_doctor(ws.as_deref())?;
            // Non-zero: store/IO failure; hooks missing stay 0
            if code != 0 {
                std::process::exit(code);
            }
            Ok(())
        }
        Command::Gc {
            workspace,
            project,
            days,
            vacuum,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::gc::cmd_gc(ws.as_deref(), days, vacuum)
        }
        Command::Migrate { subcmd } => match subcmd {
            MigrateCommand::V2 {
                workspace,
                allow_skew,
            } => kaizen::shell::migrate::cmd_migrate_v2(workspace.as_deref(), allow_skew),
            MigrateCommand::V1 { workspace } => {
                kaizen::shell::migrate::cmd_migrate_v1(workspace.as_deref())
            }
        },
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
            project,
            all_workspaces,
            refresh,
            source,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::insights::cmd_insights(ws.as_deref(), all_workspaces, refresh, source)
        }
        Command::Guidance {
            days,
            json,
            workspace,
            project,
            refresh,
            source,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::guidance::cmd_guidance(ws.as_deref(), days, json, refresh, source)
        }
        Command::Metrics {
            subcmd,
            days,
            json,
            force,
            workspace,
            project,
            all_workspaces,
            refresh,
            source,
        } => match subcmd {
            Some(MetricsCommand::Index {
                workspace,
                project,
                force,
            }) => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::metrics::cmd_metrics_index(ws.as_deref(), force)
            }
            None => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::metrics::cmd_metrics(
                    ws.as_deref(),
                    days,
                    json,
                    force,
                    all_workspaces,
                    refresh,
                    source,
                )
            }
        },
        Command::Sync {
            subcmd:
                SyncCommand::Run {
                    workspace,
                    project,
                    once,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::sync::cmd_sync_run(ws.as_deref(), once)
        }
        Command::Sync {
            subcmd: SyncCommand::Status { workspace, project },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::sync::cmd_sync_status(ws.as_deref())
        }
        Command::Telemetry { subcmd } => match subcmd {
            TelemetrySubcommand::Init {
                workspace,
                project,
                exporter_type,
                path,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry::cmd_telemetry_init(
                    ws.as_deref(),
                    kaizen::shell::telemetry::ConfigureOptions {
                        exporter_type: exporter_type.map(|t| t.as_str().to_string()),
                        path,
                        ..Default::default()
                    },
                )
            }
            TelemetrySubcommand::Doctor { workspace, project } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry::cmd_telemetry_doctor(ws.as_deref())
            }
            TelemetrySubcommand::Pull {
                days,
                workspace,
                project,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry::cmd_telemetry_pull(ws.as_deref(), days)
            }
            TelemetrySubcommand::Push {
                days,
                workspace,
                project,
                all_workspaces,
                dry_run,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry::cmd_telemetry_push(
                    ws.as_deref(),
                    all_workspaces,
                    days,
                    dry_run,
                )
            }
            TelemetrySubcommand::PrintSchema => {
                kaizen::shell::telemetry::cmd_telemetry_print_schema()
            }
            TelemetrySubcommand::Configure {
                workspace,
                project,
                exporter_type,
                path,
                api_key,
                site,
                host,
                endpoint,
                non_interactive,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry::cmd_telemetry_configure(
                    ws.as_deref(),
                    kaizen::shell::telemetry::ConfigureOptions {
                        exporter_type: exporter_type.map(|t| t.as_str().to_string()),
                        path,
                        api_key,
                        site,
                        host,
                        endpoint,
                        non_interactive,
                    },
                )
            }
            TelemetrySubcommand::Test { workspace, project } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry::cmd_telemetry_test(ws.as_deref())
            }
            TelemetrySubcommand::PrintEffectiveConfig { workspace, project } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry::cmd_telemetry_print_effective(ws.as_deref())
            }
            TelemetrySubcommand::Tail {
                workspace,
                project,
                file,
                no_follow,
                json,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::telemetry_tail::cmd_telemetry_tail(
                    ws.as_deref(),
                    file,
                    no_follow,
                    json,
                )
            }
        },
        Command::Retro {
            days,
            dry_run,
            json,
            force,
            workspace,
            project,
            refresh,
            source,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::retro::cmd_retro(
                ws.as_deref(),
                days,
                dry_run,
                json,
                force,
                refresh,
                source,
            )
        }
        Command::Projects { subcmd } => match subcmd {
            ProjectsCommand::List => kaizen::shell::projects::cmd_projects_list(),
        },
        Command::Exp { subcmd } => dispatch_exp(subcmd),
        Command::Upgrade { from_source } => kaizen::shell::upgrade::cmd_upgrade(from_source),
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
                    project,
                },
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::proxy::cmd_proxy_run(ws.as_deref(), listen, upstream)
        }
        Command::Eval { subcmd } => match subcmd {
            EvalCommand::Run {
                workspace,
                project,
                since_days,
                dry_run,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::eval::cmd_eval_run(ws.as_deref(), since_days, dry_run)
            }
            EvalCommand::List {
                workspace,
                project,
                min_score,
                json,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::eval::cmd_eval_list(ws.as_deref(), min_score, json)
            }
            EvalCommand::Prompt {
                workspace,
                project,
                session_id,
                rubric,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::eval::cmd_eval_prompt(ws.as_deref(), &session_id, &rubric)
            }
        },
        Command::Prompt { subcmd } => match subcmd {
            PromptCommand::List {
                workspace,
                project,
                json,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::prompt::cmd_prompt_list(ws.as_deref(), json)
            }
            PromptCommand::Show {
                fingerprint,
                workspace,
                project,
                json,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::prompt::cmd_prompt_show(&fingerprint, ws.as_deref(), json)
            }
            PromptCommand::Diff {
                fingerprint_a,
                fingerprint_b,
                workspace,
                project,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::prompt::cmd_prompt_diff(
                    &fingerprint_a,
                    &fingerprint_b,
                    ws.as_deref(),
                )
            }
        },
        Command::Outcomes { subcmd } => match subcmd {
            OutcomesCommand::Show {
                id,
                workspace,
                project,
            } => {
                let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
                kaizen::shell::outcomes_cmd::cmd_outcomes_show(&id, ws.as_deref())
            }
            OutcomesCommand::Measure { workspace, session } => {
                kaizen::shell::outcomes_cmd::cmd_outcomes_measure(&workspace, &session)
            }
        },
        Command::SamplerRun {
            workspace,
            session,
            pid,
        } => kaizen::shell::sampler_cmd::cmd_sampler_run(&workspace, &session, pid),
    }
}

fn dispatch_daemon(cmd: DaemonCommand) -> anyhow::Result<()> {
    match cmd {
        DaemonCommand::Start { background } => {
            if !background {
                return kaizen::daemon::start_foreground();
            }
            let started = kaizen::daemon::start_background()?;
            if started.already_running {
                println!("daemon already running");
            } else {
                println!("daemon started");
            }
            println!("pid: {}", started.pid);
            println!("socket: {}", started.paths.sock.display());
            println!("log: {}", started.paths.log.display());
            Ok(())
        }
        DaemonCommand::Stop => {
            println!("{}", kaizen::daemon::stop()?);
            Ok(())
        }
        DaemonCommand::Status => {
            match kaizen::daemon::status_outcome()? {
                kaizen::daemon::DaemonStatusOutcome::Running(st) => {
                    println!("status: running");
                    println!("pid: {}", st.pid);
                    println!("uptime_ms: {}", st.uptime_ms);
                    println!("queue_depth: {}", st.queue_depth);
                    println!(
                        "last_error: {}",
                        st.last_error.unwrap_or_else(|| "-".to_string())
                    );
                }
                kaizen::daemon::DaemonStatusOutcome::Stopped { socket } => {
                    println!("status: stopped");
                    println!("socket: {}", socket.display());
                }
            }
            Ok(())
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
            control_branch,
            treatment_branch,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_new(
                ws.as_deref(),
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
            )
        }
        ExpCommand::Start {
            id,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_start(ws.as_deref(), &id)
        }
        ExpCommand::List { workspace, project } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_list(ws.as_deref())
        }
        ExpCommand::Status {
            id,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_status(ws.as_deref(), &id)
        }
        ExpCommand::Tag {
            id,
            session,
            variant,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_tag(ws.as_deref(), &id, &session, &variant)
        }
        ExpCommand::Report {
            id,
            json,
            refresh,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_report(ws.as_deref(), &id, json, refresh)
        }
        ExpCommand::Conclude {
            id,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_conclude(ws.as_deref(), &id)
        }
        ExpCommand::Archive {
            id,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_archive(ws.as_deref(), &id)
        }
        ExpCommand::Power {
            metric,
            baseline_n,
            refresh,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_power(ws.as_deref(), &metric, baseline_n, refresh)
        }
    }
}

fn ingest_hook(source: Source, workspace: Option<PathBuf>) -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let src = match source {
        Source::Cursor => kaizen::shell::ingest::IngestSource::Cursor,
        Source::Claude => kaizen::shell::ingest::IngestSource::Claude,
        Source::Vibe => kaizen::shell::ingest::IngestSource::Vibe,
    };
    if kaizen::daemon::enabled() {
        let response = kaizen::daemon::request_blocking(kaizen::ipc::DaemonRequest::IngestHook {
            source: src,
            payload: input,
            workspace: workspace.map(|p| {
                kaizen::core::paths::canonical(&p)
                    .to_string_lossy()
                    .to_string()
            }),
        })?;
        return match response {
            kaizen::ipc::DaemonResponse::Ack { .. } => Ok(()),
            kaizen::ipc::DaemonResponse::Error { message, .. } => Err(anyhow::anyhow!(message)),
            _ => Err(anyhow::anyhow!("unexpected daemon ingest response")),
        };
    }
    kaizen::shell::ingest::ingest_hook_text(src, &input, workspace)
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
