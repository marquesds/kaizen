use super::*;

#[derive(Subcommand)]
pub(crate) enum ProxyCommand {
    /// Bind and forward until interrupted.
    Run {
        /// Address to listen, e.g. 127.0.0.1:3847 (overrides [proxy] in config TOML).
        #[arg(long)]
        listen: Option<String>,
        /// Upstream base URL, e.g. https://api.anthropic.com (no trailing slash).
        #[arg(long)]
        upstream: Option<String>,
        /// Provider defaults and hints: anthropic, openai, or auto.
        #[arg(long)]
        provider: Option<String>,
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub(crate) enum TelemetrySubcommand {
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
        /// File exporter path, absolute or relative to Kaizen project data.
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
        /// File exporter path, absolute or relative to Kaizen project data.
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
    /// Read local NDJSON from the `file` exporter under Kaizen project data.
    Tail {
        /// workspace root (default: cwd)
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// project name shorthand for --workspace (mutually exclusive)
        #[arg(long, conflicts_with = "workspace")]
        project: Option<String>,
        /// File path (absolute or relative to Kaizen project data).
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
pub(crate) enum TelemetryExporterKind {
    File,
    Posthog,
    Datadog,
    Otlp,
    Dev,
}

impl TelemetryExporterKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Posthog => "posthog",
            Self::Datadog => "datadog",
            Self::Otlp => "otlp",
            Self::Dev => "dev",
        }
    }
}
