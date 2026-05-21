// SPDX-License-Identifier: AGPL-3.0-or-later
//! Config loading: workspace `.kaizen/config.toml` then `~/.kaizen/config.toml`.
//! Missing files → defaults. User config wins on overlap.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanConfig {
    pub roots: Vec<String>,
    /// Minimum seconds between full agent transcript rescans when `--refresh` is not passed.
    #[serde(default = "default_min_rescan_seconds")]
    pub min_rescan_seconds: u64,
}

fn default_min_rescan_seconds() -> u64 {
    300
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            roots: vec!["~/.cursor/projects".to_string()],
            min_rescan_seconds: default_min_rescan_seconds(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorSourceConfig {
    pub enabled: bool,
    pub transcript_glob: String,
}

impl Default for CursorSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            transcript_glob: "*/agent-transcripts".to_string(),
        }
    }
}

/// Enable tier-1 tail ingestion for agents that store data outside Cursor/Claude/Codex paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailAgentToggles {
    #[serde(default = "default_true")]
    pub goose: bool,
    #[serde(default = "default_true")]
    pub openclaw: bool,
    #[serde(default = "default_true")]
    pub opencode: bool,
    #[serde(default = "default_true")]
    pub copilot_cli: bool,
    #[serde(default = "default_true")]
    pub copilot_vscode: bool,
}

impl Default for TailAgentToggles {
    fn default() -> Self {
        Self {
            goose: true,
            openclaw: true,
            opencode: true,
            copilot_cli: true,
            copilot_vscode: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourcesConfig {
    #[serde(default)]
    pub cursor: CursorSourceConfig,
    #[serde(default)]
    pub tail: TailAgentToggles,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionConfig {
    pub hot_days: u32,
    pub warm_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            hot_days: 30,
            warm_days: 90,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageConfig {
    pub hot_max_bytes: String,
    pub cold_after_days: u32,
    pub retention_days: u32,
    pub flush_hour_utc: u8,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            hot_max_bytes: "1GB".into(),
            cold_after_days: 7,
            retention_days: 90,
            flush_hour_utc: 0,
        }
    }
}

impl StorageConfig {
    pub fn hot_max_bytes_value(&self) -> u64 {
        parse_byte_size(&self.hot_max_bytes).unwrap_or(1_073_741_824)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// When empty, sync is disabled (no outbox enqueue, `sync run` no-ops flush).
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub team_token: String,
    #[serde(default)]
    pub team_id: String,
    #[serde(default = "default_events_per_batch")]
    pub events_per_batch_max: usize,
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: usize,
    #[serde(default = "default_flush_interval_ms")]
    pub flush_interval_ms: u64,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
    /// 64 hex chars (32 bytes). Prefer `~/.kaizen/config.toml` only; never committed workspace secrets.
    #[serde(default)]
    pub team_salt_hex: String,
}

fn default_events_per_batch() -> usize {
    500
}

fn default_max_body_bytes() -> usize {
    1_000_000
}

fn default_flush_interval_ms() -> u64 {
    10_000
}

fn default_sample_rate() -> f64 {
    1.0
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            team_token: String::new(),
            team_id: String::new(),
            events_per_batch_max: default_events_per_batch(),
            max_body_bytes: default_max_body_bytes(),
            flush_interval_ms: default_flush_interval_ms(),
            sample_rate: default_sample_rate(),
            team_salt_hex: String::new(),
        }
    }
}

/// Parse `team_salt_hex` into 32 bytes. Returns `None` if missing or invalid.
pub fn try_team_salt(cfg: &SyncConfig) -> Option<[u8; 32]> {
    let h = cfg.team_salt_hex.trim();
    if h.len() != 64 {
        return None;
    }
    let bytes = hex::decode(h).ok()?;
    bytes.try_into().ok()
}

/// Resolve a 32-byte redaction salt for telemetry-only flows (push/test) when sync is not
/// configured. Order: configured `[sync].team_salt_hex` → `<kaizen_home>/local_salt.hex`
/// → freshly generated and persisted at `0o600`. Telemetry never blocks on cloud sync.
pub fn effective_redaction_salt(
    cfg: &SyncConfig,
    kaizen_home: &std::path::Path,
) -> Result<[u8; 32]> {
    if let Some(s) = try_team_salt(cfg) {
        return Ok(s);
    }
    let path = kaizen_home.join("local_salt.hex");
    if let Some(s) = read_local_salt(&path)? {
        return Ok(s);
    }
    let bytes = generate_local_salt();
    write_local_salt(&path, &bytes)?;
    Ok(bytes)
}

fn read_local_salt(path: &std::path::Path) -> Result<Option<[u8; 32]>> {
    use std::io::ErrorKind;
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(parse_salt_hex(s.trim())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn parse_salt_hex(h: &str) -> Option<[u8; 32]> {
    if h.len() != 64 {
        return None;
    }
    hex::decode(h).ok()?.try_into().ok()
}

fn generate_local_salt() -> [u8; 32] {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes
}

fn write_local_salt(path: &std::path::Path, bytes: &[u8; 32]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let hex_s = hex::encode(bytes);
    std::fs::write(path, hex_s.as_bytes())?;
    set_user_only_perms(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_user_only_perms(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_user_only_perms(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

fn default_true() -> bool {
    true
}

fn default_telemetry_fail_open() -> bool {
    true
}

fn default_cache_ttl_seconds() -> u64 {
    3600
}

/// Which third-party system is the single source for query-back / pull; OTLP is export-only, not a pull target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryAuthority {
    #[default]
    None,
    Posthog,
    Datadog,
}

/// Per-field allowlist: when `false` (default), the field is omitted or hashed in telemetry exports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityAllowlist {
    #[serde(default)]
    pub team: bool,
    #[serde(default)]
    pub workspace_label: bool,
    #[serde(default)]
    pub runner_label: bool,
    #[serde(default)]
    pub actor_kind: bool,
    #[serde(default)]
    pub actor_label: bool,
    #[serde(default)]
    pub agent: bool,
    #[serde(default)]
    pub model: bool,
    #[serde(default)]
    pub env: bool,
    #[serde(default)]
    pub job: bool,
    #[serde(default)]
    pub branch: bool,
}

/// Remote pull: query authority, cache TTL, and which identity labels may leave as cleartext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryQueryConfig {
    /// `posthog` or `datadog` enables provider pull when implemented; `none` or unset = no query authority.
    #[serde(default)]
    pub provider: QueryAuthority,
    /// Seconds to treat remote cache rows as fresh (unless the CLI requests `--refresh`).
    #[serde(default = "default_cache_ttl_seconds")]
    pub cache_ttl_seconds: u64,
    #[serde(default)]
    pub identity_allowlist: IdentityAllowlist,
}

impl Default for TelemetryQueryConfig {
    fn default() -> Self {
        Self {
            provider: QueryAuthority::default(),
            cache_ttl_seconds: default_cache_ttl_seconds(),
            identity_allowlist: IdentityAllowlist::default(),
        }
    }
}

impl TelemetryQueryConfig {
    /// True when a PostHog or Datadog pull backend may be used (OTLP is not a pull target).
    pub fn has_provider_for_pull(&self) -> bool {
        matches!(
            self.provider,
            QueryAuthority::Posthog | QueryAuthority::Datadog
        )
    }
}

/// How to reduce billed input to the model (opt-in; default leaves requests unchanged).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextPolicy {
    /// No transformation beyond optional JSON minify (same tokens as a direct call).
    #[default]
    None,
    /// Keep the last `count` `messages` array entries; system blocks unchanged when present.
    LastMessages { count: usize },
    /// Drop oldest messages until a rough `chars/4` estimate stays at or below `max`.
    MaxInputTokens { max: u32 },
}

/// Anthropic API-compatible HTTP proxy: forward + local telemetry. See `docs/llm-proxy.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// e.g. `127.0.0.1:3847` (bind address for `kaizen proxy run`).
    #[serde(default = "default_proxy_listen")]
    pub listen: String,
    /// Base URL, no trailing slash, e.g. `https://api.anthropic.com`.
    #[serde(default = "default_proxy_upstream")]
    pub upstream: String,
    /// `anthropic`, `openai`, or `auto`; controls launcher/env hints and default upstream.
    #[serde(default = "default_proxy_provider")]
    pub provider: String,
    /// Prefer `Accept-Encoding: gzip` to upstream (response bodies may be gzip).
    #[serde(default = "default_true")]
    pub compress_transport: bool,
    /// Re-encode JSON bodies to compact `serde_json` (no key reorder; whitespace only).
    #[serde(default = "default_true")]
    pub minify_json: bool,
    /// Slurp cap for a single upstream response (streaming not yet teed; see doc).
    #[serde(default = "default_proxy_max_body_mb")]
    pub max_response_body_mb: u32,
    /// Reject / fail incoming client bodies above this (POST bodies before forward).
    #[serde(default = "default_proxy_max_request_body_mb")]
    pub max_request_body_mb: u32,
    /// Optional token-aware truncation of `messages` in JSON bodies.
    #[serde(default)]
    pub context_policy: ContextPolicy,
}

fn default_proxy_listen() -> String {
    "127.0.0.1:3847".to_string()
}

fn default_proxy_upstream() -> String {
    "https://api.anthropic.com".to_string()
}

fn default_proxy_provider() -> String {
    "anthropic".to_string()
}

fn default_proxy_max_body_mb() -> u32 {
    256
}

fn default_proxy_max_request_body_mb() -> u32 {
    32
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            listen: default_proxy_listen(),
            upstream: default_proxy_upstream(),
            provider: default_proxy_provider(),
            compress_transport: true,
            minify_json: true,
            max_response_body_mb: default_proxy_max_body_mb(),
            max_request_body_mb: default_proxy_max_request_body_mb(),
            context_policy: ContextPolicy::default(),
        }
    }
}

/// Optional third-party telemetry sinks; same redacted batches as Kaizen sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// When `true` (default), ignore exporter errors; when `false`, `flush` fails if any secondary errors.
    #[serde(default = "default_telemetry_fail_open")]
    pub fail_open: bool,
    /// Query-back / pull API: authority, cache TTL, identity allowlist.
    #[serde(default)]
    pub query: TelemetryQueryConfig,
    /// Declarative list; `type = "none"` rows are accepted and ignored.
    #[serde(default)]
    pub exporters: Vec<ExporterConfig>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            fail_open: default_telemetry_fail_open(),
            query: TelemetryQueryConfig::default(),
            exporters: Vec::new(),
        }
    }
}

/// One pluggable sink; TOML `type` is the tag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExporterConfig {
    /// No-op row for sparse tables / templates.
    None,
    /// Append summary JSON lines to a local NDJSON file (default `<workspace>/.kaizen/telemetry.ndjson`).
    File {
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default)]
        path: Option<String>,
    },
    /// Echo to tracing (for wiring tests; requires the `telemetry-dev` build feature).
    Dev {
        #[serde(default = "default_true")]
        enabled: bool,
    },
    PostHog {
        #[serde(default = "default_true")]
        enabled: bool,
        /// e.g. `https://us.i.posthog.com` (default when unset)
        host: Option<String>,
        /// Prefer env `POSTHOG_API_KEY` or `KAIZEN_POSTHOG_API_KEY`
        project_api_key: Option<String>,
    },
    Datadog {
        #[serde(default = "default_true")]
        enabled: bool,
        /// e.g. `datadoghq.com`; env `DD_SITE` overrides
        site: Option<String>,
        /// Prefer env `DD_API_KEY` or `KAIZEN_DD_API_KEY`
        api_key: Option<String>,
    },
    Otlp {
        #[serde(default = "default_true")]
        enabled: bool,
        /// Env `OTEL_EXPORTER_OTLP_ENDPOINT` (or KAIZEN_ prefix) when unset here
        endpoint: Option<String>,
    },
}

impl ExporterConfig {
    /// Whether this row should be considered for `load_exporters` (excludes `None`).
    pub fn is_enabled(&self) -> bool {
        match self {
            ExporterConfig::None => false,
            ExporterConfig::File { enabled, .. } => *enabled,
            ExporterConfig::Dev { enabled, .. } => *enabled,
            ExporterConfig::PostHog { enabled, .. } => *enabled,
            ExporterConfig::Datadog { enabled, .. } => *enabled,
            ExporterConfig::Otlp { enabled, .. } => *enabled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_eval_endpoint")]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_eval_model")]
    pub model: String,
    #[serde(default = "default_eval_rubric")]
    pub rubric: String,
    #[serde(default = "default_eval_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_eval_min_cost")]
    pub min_cost_usd: f64,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_eval_endpoint(),
            api_key: String::new(),
            model: default_eval_model(),
            rubric: default_eval_rubric(),
            batch_size: default_eval_batch_size(),
            min_cost_usd: default_eval_min_cost(),
        }
    }
}

fn default_eval_endpoint() -> String {
    "https://api.anthropic.com".into()
}
fn default_eval_model() -> String {
    "claude-haiku-4-5-20251001".into()
}
fn default_eval_rubric() -> String {
    "tool-efficiency-v1".into()
}
fn default_eval_batch_size() -> usize {
    20
}
fn default_eval_min_cost() -> f64 {
    0.01
}

/// Opt-in post-hook outcome measurement (Tier C).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectOutcomesConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_outcomes_test_cmd")]
    pub test_cmd: String,
    #[serde(default = "default_outcomes_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub lint_cmd: Option<String>,
}

fn default_outcomes_test_cmd() -> String {
    "cargo test --quiet".to_string()
}

fn default_outcomes_timeout_secs() -> u64 {
    600
}

impl Default for CollectOutcomesConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            test_cmd: default_outcomes_test_cmd(),
            timeout_secs: default_outcomes_timeout_secs(),
            lint_cmd: None,
        }
    }
}

/// Opt-in per-process sampling (Tier D).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectSystemSamplerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_sampler_sample_ms")]
    pub sample_ms: u64,
    #[serde(default = "default_sampler_max_samples")]
    pub max_samples_per_session: u32,
}

fn default_sampler_sample_ms() -> u64 {
    2000
}

fn default_sampler_max_samples() -> u32 {
    3600
}

impl Default for CollectSystemSamplerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sample_ms: default_sampler_sample_ms(),
            max_samples_per_session: default_sampler_max_samples(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectConfig {
    #[serde(default)]
    pub outcomes: CollectOutcomesConfig,
    #[serde(default)]
    pub system_sampler: CollectSystemSamplerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub sources: SourcesConfig,
    #[serde(default)]
    pub retention: RetentionConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub eval: EvalConfig,
    #[serde(default)]
    pub collect: CollectConfig,
}

/// Load config: `~/.kaizen/projects/<slug>/config.toml` then `~/.kaizen/config.toml`.
/// User config wins on overlap. Missing files → defaults, not error.
pub fn load(workspace: &Path) -> Result<Config> {
    let project_cfg = crate::core::paths::project_data_dir(workspace)
        .ok()
        .map(|d| d.join("config.toml"));
    let user_path = crate::core::paths::kaizen_dir()
        .ok_or_else(|| anyhow::anyhow!("KAIZEN_HOME / HOME unset"))?
        .join("config.toml");

    let base = project_cfg
        .as_deref()
        .and_then(load_file)
        .unwrap_or_default();
    let user = load_file(&user_path).unwrap_or_default();
    Ok(merge(base, user))
}

fn load_file(path: &Path) -> Option<Config> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

fn merge(base: Config, user: Config) -> Config {
    Config {
        scan: merge_scan(base.scan, user.scan),
        sources: merge_sources(base.sources, user.sources),
        retention: merge_retention(base.retention, user.retention),
        storage: merge_storage(base.storage, user.storage),
        sync: merge_sync(base.sync, user.sync),
        telemetry: merge_telemetry(base.telemetry, user.telemetry),
        proxy: merge_proxy(base.proxy, user.proxy),
        eval: merge_eval(base.eval, user.eval),
        collect: merge_collect(base.collect, user.collect),
    }
}

fn merge_collect(base: CollectConfig, user: CollectConfig) -> CollectConfig {
    let def = CollectConfig::default();
    CollectConfig {
        outcomes: merge_collect_outcomes(base.outcomes, user.outcomes, def.outcomes),
        system_sampler: merge_collect_sampler(
            base.system_sampler,
            user.system_sampler,
            def.system_sampler,
        ),
    }
}

fn merge_collect_outcomes(
    base: CollectOutcomesConfig,
    user: CollectOutcomesConfig,
    def: CollectOutcomesConfig,
) -> CollectOutcomesConfig {
    CollectOutcomesConfig {
        enabled: if user.enabled != def.enabled {
            user.enabled
        } else {
            base.enabled
        },
        test_cmd: if user.test_cmd != def.test_cmd {
            user.test_cmd
        } else {
            base.test_cmd
        },
        timeout_secs: if user.timeout_secs != def.timeout_secs {
            user.timeout_secs
        } else {
            base.timeout_secs
        },
        lint_cmd: user.lint_cmd.or(base.lint_cmd),
    }
}

fn merge_collect_sampler(
    base: CollectSystemSamplerConfig,
    user: CollectSystemSamplerConfig,
    def: CollectSystemSamplerConfig,
) -> CollectSystemSamplerConfig {
    CollectSystemSamplerConfig {
        enabled: if user.enabled != def.enabled {
            user.enabled
        } else {
            base.enabled
        },
        sample_ms: if user.sample_ms != def.sample_ms {
            user.sample_ms
        } else {
            base.sample_ms
        },
        max_samples_per_session: if user.max_samples_per_session != def.max_samples_per_session {
            user.max_samples_per_session
        } else {
            base.max_samples_per_session
        },
    }
}

fn merge_sources(base: SourcesConfig, user: SourcesConfig) -> SourcesConfig {
    let def = SourcesConfig::default();
    SourcesConfig {
        cursor: merge_cursor_source(base.cursor, user.cursor, def.cursor),
        tail: merge_tail_toggles(base.tail, user.tail, def.tail),
    }
}

fn merge_cursor_source(
    base: CursorSourceConfig,
    user: CursorSourceConfig,
    def: CursorSourceConfig,
) -> CursorSourceConfig {
    CursorSourceConfig {
        enabled: if user.enabled != def.enabled {
            user.enabled
        } else {
            base.enabled
        },
        transcript_glob: if user.transcript_glob != def.transcript_glob {
            user.transcript_glob
        } else {
            base.transcript_glob
        },
    }
}

fn merge_tail_toggles(
    base: TailAgentToggles,
    user: TailAgentToggles,
    def: TailAgentToggles,
) -> TailAgentToggles {
    TailAgentToggles {
        goose: if user.goose != def.goose {
            user.goose
        } else {
            base.goose
        },
        openclaw: if user.openclaw != def.openclaw {
            user.openclaw
        } else {
            base.openclaw
        },
        opencode: if user.opencode != def.opencode {
            user.opencode
        } else {
            base.opencode
        },
        copilot_cli: if user.copilot_cli != def.copilot_cli {
            user.copilot_cli
        } else {
            base.copilot_cli
        },
        copilot_vscode: if user.copilot_vscode != def.copilot_vscode {
            user.copilot_vscode
        } else {
            base.copilot_vscode
        },
    }
}

fn merge_eval(base: EvalConfig, user: EvalConfig) -> EvalConfig {
    let def = EvalConfig::default();
    EvalConfig {
        enabled: if user.enabled != def.enabled {
            user.enabled
        } else {
            base.enabled
        },
        endpoint: if user.endpoint != def.endpoint {
            user.endpoint
        } else {
            base.endpoint
        },
        api_key: if !user.api_key.is_empty() {
            user.api_key
        } else {
            base.api_key
        },
        model: if user.model != def.model {
            user.model
        } else {
            base.model
        },
        rubric: if user.rubric != def.rubric {
            user.rubric
        } else {
            base.rubric
        },
        batch_size: if user.batch_size != def.batch_size {
            user.batch_size
        } else {
            base.batch_size
        },
        min_cost_usd: if user.min_cost_usd != def.min_cost_usd {
            user.min_cost_usd
        } else {
            base.min_cost_usd
        },
    }
}

fn merge_scan(base: ScanConfig, user: ScanConfig) -> ScanConfig {
    let def = ScanConfig::default();
    ScanConfig {
        roots: if user.roots != def.roots {
            user.roots
        } else {
            base.roots
        },
        min_rescan_seconds: if user.min_rescan_seconds != def.min_rescan_seconds {
            user.min_rescan_seconds
        } else {
            base.min_rescan_seconds
        },
    }
}

fn merge_retention(base: RetentionConfig, user: RetentionConfig) -> RetentionConfig {
    let def = RetentionConfig::default();
    RetentionConfig {
        hot_days: if user.hot_days != def.hot_days {
            user.hot_days
        } else {
            base.hot_days
        },
        warm_days: if user.warm_days != def.warm_days {
            user.warm_days
        } else {
            base.warm_days
        },
    }
}

fn merge_storage(base: StorageConfig, user: StorageConfig) -> StorageConfig {
    let def = StorageConfig::default();
    StorageConfig {
        hot_max_bytes: if user.hot_max_bytes != def.hot_max_bytes {
            user.hot_max_bytes
        } else {
            base.hot_max_bytes
        },
        cold_after_days: if user.cold_after_days != def.cold_after_days {
            user.cold_after_days
        } else {
            base.cold_after_days
        },
        retention_days: if user.retention_days != def.retention_days {
            user.retention_days
        } else {
            base.retention_days
        },
        flush_hour_utc: if user.flush_hour_utc != def.flush_hour_utc {
            user.flush_hour_utc
        } else {
            base.flush_hour_utc
        },
    }
}

fn parse_byte_size(raw: &str) -> Option<u64> {
    let s = raw.trim();
    let digits = s
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    let n = digits.parse::<u64>().ok()?;
    let unit = s[digits.len()..].trim().to_ascii_lowercase();
    Some(match unit.as_str() {
        "" | "b" => n,
        "kb" | "kib" => n.saturating_mul(1024),
        "mb" | "mib" => n.saturating_mul(1024 * 1024),
        "gb" | "gib" => n.saturating_mul(1024 * 1024 * 1024),
        _ => return None,
    })
}

fn merge_proxy(base: ProxyConfig, user: ProxyConfig) -> ProxyConfig {
    let def = ProxyConfig::default();
    ProxyConfig {
        listen: if user.listen != def.listen {
            user.listen
        } else {
            base.listen
        },
        upstream: if user.upstream != def.upstream {
            user.upstream
        } else {
            base.upstream
        },
        provider: if user.provider != def.provider {
            user.provider
        } else {
            base.provider
        },
        compress_transport: if user.compress_transport != def.compress_transport {
            user.compress_transport
        } else {
            base.compress_transport
        },
        minify_json: if user.minify_json != def.minify_json {
            user.minify_json
        } else {
            base.minify_json
        },
        max_response_body_mb: if user.max_response_body_mb != def.max_response_body_mb {
            user.max_response_body_mb
        } else {
            base.max_response_body_mb
        },
        max_request_body_mb: if user.max_request_body_mb != def.max_request_body_mb {
            user.max_request_body_mb
        } else {
            base.max_request_body_mb
        },
        context_policy: if user.context_policy != def.context_policy {
            user.context_policy
        } else {
            base.context_policy
        },
    }
}

fn merge_telemetry(base: TelemetryConfig, user: TelemetryConfig) -> TelemetryConfig {
    let def = TelemetryConfig::default();
    let fail_open = if user.fail_open != def.fail_open {
        user.fail_open
    } else {
        base.fail_open
    };
    let query = merge_telemetry_query(base.query, user.query);
    let exporters = if !user.exporters.is_empty() {
        user.exporters
    } else {
        base.exporters
    };
    TelemetryConfig {
        fail_open,
        query,
        exporters,
    }
}

fn merge_telemetry_query(
    base: TelemetryQueryConfig,
    user: TelemetryQueryConfig,
) -> TelemetryQueryConfig {
    let def = TelemetryQueryConfig::default();
    TelemetryQueryConfig {
        provider: if user.provider != def.provider {
            user.provider
        } else {
            base.provider
        },
        cache_ttl_seconds: if user.cache_ttl_seconds != def.cache_ttl_seconds {
            user.cache_ttl_seconds
        } else {
            base.cache_ttl_seconds
        },
        identity_allowlist: merge_identity_allowlist(
            base.identity_allowlist,
            user.identity_allowlist,
        ),
    }
}

fn merge_identity_allowlist(base: IdentityAllowlist, user: IdentityAllowlist) -> IdentityAllowlist {
    let def = IdentityAllowlist::default();
    IdentityAllowlist {
        team: if user.team != def.team {
            user.team
        } else {
            base.team
        },
        workspace_label: if user.workspace_label != def.workspace_label {
            user.workspace_label
        } else {
            base.workspace_label
        },
        runner_label: if user.runner_label != def.runner_label {
            user.runner_label
        } else {
            base.runner_label
        },
        actor_kind: if user.actor_kind != def.actor_kind {
            user.actor_kind
        } else {
            base.actor_kind
        },
        actor_label: if user.actor_label != def.actor_label {
            user.actor_label
        } else {
            base.actor_label
        },
        agent: if user.agent != def.agent {
            user.agent
        } else {
            base.agent
        },
        model: if user.model != def.model {
            user.model
        } else {
            base.model
        },
        env: if user.env != def.env {
            user.env
        } else {
            base.env
        },
        job: if user.job != def.job {
            user.job
        } else {
            base.job
        },
        branch: if user.branch != def.branch {
            user.branch
        } else {
            base.branch
        },
    }
}

fn merge_sync(base: SyncConfig, user: SyncConfig) -> SyncConfig {
    let def = SyncConfig::default();
    SyncConfig {
        endpoint: if !user.endpoint.is_empty() {
            user.endpoint
        } else {
            base.endpoint
        },
        team_token: if !user.team_token.is_empty() {
            user.team_token
        } else {
            base.team_token
        },
        team_id: if !user.team_id.is_empty() {
            user.team_id
        } else {
            base.team_id
        },
        events_per_batch_max: if user.events_per_batch_max != def.events_per_batch_max {
            user.events_per_batch_max
        } else {
            base.events_per_batch_max
        },
        max_body_bytes: if user.max_body_bytes != def.max_body_bytes {
            user.max_body_bytes
        } else {
            base.max_body_bytes
        },
        flush_interval_ms: if user.flush_interval_ms != def.flush_interval_ms {
            user.flush_interval_ms
        } else {
            base.flush_interval_ms
        },
        sample_rate: if (user.sample_rate - def.sample_rate).abs() > f64::EPSILON {
            user.sample_rate
        } else {
            base.sample_rate
        },
        team_salt_hex: if !user.team_salt_hex.is_empty() {
            user.team_salt_hex
        } else {
            base.team_salt_hex
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn defaults_when_no_files() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.scan.roots, ScanConfig::default().roots);
        assert_eq!(cfg.scan.min_rescan_seconds, 300);
        assert_eq!(cfg.retention.hot_days, 30);
        assert_eq!(cfg.storage.cold_after_days, 7);
        assert_eq!(cfg.storage.hot_max_bytes_value(), 1_073_741_824);
    }

    #[test]
    fn effective_redaction_salt_prefers_configured_team_salt() {
        let home = TempDir::new().unwrap();
        let sync = SyncConfig {
            team_salt_hex: "ab".repeat(32),
            ..Default::default()
        };
        let salt = effective_redaction_salt(&sync, home.path()).unwrap();
        assert_eq!(salt, [0xab_u8; 32]);
        // No local file written when team salt was sufficient.
        assert!(!home.path().join("local_salt.hex").exists());
    }

    #[test]
    fn effective_redaction_salt_generates_and_persists_local_salt() {
        let home = TempDir::new().unwrap();
        let sync = SyncConfig::default();
        let a = effective_redaction_salt(&sync, home.path()).unwrap();
        let b = effective_redaction_salt(&sync, home.path()).unwrap();
        assert_eq!(a, b, "second call must reuse the persisted local salt");
        assert!(home.path().join("local_salt.hex").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(home.path().join("local_salt.hex"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn workspace_config_loaded() {
        let _guard = crate::core::paths::test_lock::global().lock().unwrap();
        let home = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
        let data_dir = crate::core::paths::project_data_dir(ws.path()).unwrap();
        let mut f = std::fs::File::create(data_dir.join("config.toml")).unwrap();
        writeln!(f, "[scan]\nroots = [\"/custom/root\"]").unwrap();
        let cfg = load(ws.path()).unwrap();
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        assert_eq!(cfg.scan.roots, vec!["/custom/root"]);
    }

    #[test]
    fn invalid_toml_ignored() {
        let _guard = crate::core::paths::test_lock::global().lock().unwrap();
        let home = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
        let data_dir = crate::core::paths::project_data_dir(ws.path()).unwrap();
        std::fs::write(data_dir.join("config.toml"), "not valid toml :::").unwrap();
        let cfg = load(ws.path()).unwrap();
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        assert_eq!(cfg.scan.roots, ScanConfig::default().roots);
    }

    #[test]
    fn merge_user_roots_win() {
        let base = Config {
            scan: ScanConfig {
                roots: vec!["/base".to_string()],
                ..ScanConfig::default()
            },
            ..Default::default()
        };
        let user = Config {
            scan: ScanConfig {
                roots: vec!["/user".to_string()],
                ..ScanConfig::default()
            },
            ..Default::default()
        };
        let merged = merge(base, user);
        assert_eq!(merged.scan.roots, vec!["/user"]);
    }

    #[test]
    fn merge_sources_user_default_keeps_workspace_cursor() {
        let base = Config {
            sources: SourcesConfig {
                cursor: CursorSourceConfig {
                    enabled: false,
                    transcript_glob: "/workspace/glob/**".into(),
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let user = Config::default();
        let merged = merge(base, user);
        assert!(!merged.sources.cursor.enabled);
        assert_eq!(merged.sources.cursor.transcript_glob, "/workspace/glob/**");
    }

    #[test]
    fn merge_retention_field_by_field() {
        let base = Config {
            retention: RetentionConfig {
                hot_days: 60,
                warm_days: 90,
            },
            ..Default::default()
        };
        let user = Config {
            retention: RetentionConfig {
                hot_days: 30,
                warm_days: 45,
            },
            ..Default::default()
        };
        let merged = merge(base, user);
        assert_eq!(merged.retention.hot_days, 60);
        assert_eq!(merged.retention.warm_days, 45);
    }

    #[test]
    fn merge_retention_user_hot_overrides() {
        let base = Config {
            retention: RetentionConfig {
                hot_days: 60,
                warm_days: 90,
            },
            ..Default::default()
        };
        let user = Config {
            retention: RetentionConfig {
                hot_days: 14,
                warm_days: 90,
            },
            ..Default::default()
        };
        let merged = merge(base, user);
        assert_eq!(merged.retention.hot_days, 14);
        assert_eq!(merged.retention.warm_days, 90);
    }

    #[test]
    fn merge_storage_user_overrides() {
        let base = Config {
            storage: StorageConfig {
                hot_max_bytes: "2GB".into(),
                cold_after_days: 14,
                retention_days: 120,
                flush_hour_utc: 3,
            },
            ..Default::default()
        };
        let user = Config {
            storage: StorageConfig {
                cold_after_days: 3,
                ..StorageConfig::default()
            },
            ..Default::default()
        };
        let merged = merge(base, user);
        assert_eq!(merged.storage.hot_max_bytes, "2GB");
        assert_eq!(merged.storage.cold_after_days, 3);
        assert_eq!(merged.storage.retention_days, 120);
        assert_eq!(merged.storage.flush_hour_utc, 3);
    }

    #[test]
    fn merge_telemetry_exporters_user_wins_non_empty() {
        let base = Config {
            telemetry: TelemetryConfig {
                fail_open: true,
                query: TelemetryQueryConfig::default(),
                exporters: vec![ExporterConfig::None],
            },
            ..Default::default()
        };
        let user = Config {
            telemetry: TelemetryConfig {
                fail_open: false,
                query: TelemetryQueryConfig::default(),
                exporters: vec![ExporterConfig::Dev { enabled: true }],
            },
            ..Default::default()
        };
        let merged = merge(base, user);
        assert!(!merged.telemetry.fail_open);
        assert_eq!(merged.telemetry.exporters.len(), 1);
    }

    #[test]
    fn telemetry_query_defaults() {
        let t = TelemetryQueryConfig::default();
        assert_eq!(t.provider, QueryAuthority::None);
        assert_eq!(t.cache_ttl_seconds, 3600);
        assert!(!t.identity_allowlist.team);
        assert!(!t.has_provider_for_pull());
    }

    #[test]
    fn telemetry_query_has_provider() {
        let ph = TelemetryQueryConfig {
            provider: QueryAuthority::Posthog,
            ..Default::default()
        };
        assert!(ph.has_provider_for_pull());
        let dd = TelemetryQueryConfig {
            provider: QueryAuthority::Datadog,
            ..Default::default()
        };
        assert!(dd.has_provider_for_pull());
    }

    #[test]
    fn merge_telemetry_query_user_wins() {
        let base = Config {
            telemetry: TelemetryConfig {
                query: TelemetryQueryConfig {
                    provider: QueryAuthority::Posthog,
                    cache_ttl_seconds: 3600,
                    identity_allowlist: IdentityAllowlist {
                        team: true,
                        ..Default::default()
                    },
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let user = Config {
            telemetry: TelemetryConfig {
                query: TelemetryQueryConfig {
                    cache_ttl_seconds: 7200,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let merged = merge(base, user);
        assert_eq!(merged.telemetry.query.provider, QueryAuthority::Posthog);
        assert_eq!(merged.telemetry.query.cache_ttl_seconds, 7200);
        assert!(merged.telemetry.query.identity_allowlist.team);
    }

    #[test]
    fn toml_telemetry_query_roundtrip() {
        let _guard = crate::core::paths::test_lock::global().lock().unwrap();
        let home = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
        let data_dir = crate::core::paths::project_data_dir(ws.path()).unwrap();
        let toml = r#"
[telemetry.query]
provider = "datadog"
cache_ttl_seconds = 1800

[telemetry.query.identity_allowlist]
team = true
branch = true
"#;
        std::fs::write(data_dir.join("config.toml"), toml).unwrap();
        let cfg = load(ws.path()).unwrap();
        unsafe { std::env::remove_var("KAIZEN_HOME") };
        assert_eq!(cfg.telemetry.query.provider, QueryAuthority::Datadog);
        assert_eq!(cfg.telemetry.query.cache_ttl_seconds, 1800);
        assert!(cfg.telemetry.query.identity_allowlist.team);
        assert!(cfg.telemetry.query.identity_allowlist.branch);
        assert!(!cfg.telemetry.query.identity_allowlist.model);
    }
}
