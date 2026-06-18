// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP `#[tool]` handlers (stdio server).

use crate::core::data_source::DataSource;
use crate::shell::exp::NewArgs;
use crate::shell::ingest::{IngestSource, ingest_hook_string};
use crate::shell::{cli, exp, init, insights, metrics, retro, sync};
use rmcp::ServerHandler;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ErrorData};
use rmcp::schemars;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use serde::Deserialize;

/// Static help for model routing (keep in sync with `kaizen --help` groups).
const MCP_CAPABILITIES: &str = r#"Kaizen MCP exposes most `kaizen` CLI workflows as tools. Shell-only today: doctor, guidance, gc, completions, proxy run, telemetry subcommands (init, doctor, pull, print-schema, configure, print-effective-config).

- kaizen_summary — Session counts, USD cost, by-agent/model, top tools. Use for spend and volume. Optional json=true.
- kaizen_metrics — Code hotspots, slow tools (p95), token-heavy tools, churn. Use for **repository** and tool latency. Optional json.
- kaizen_sessions_list / kaizen_session_show — Session list and one session metadata. Optional json on list; optional `limit` caps rows (newest first). `kaizen_exp_report` supports `refresh: true` for a full transcript rescan before computing the report (matches CLI `kaizen exp report --refresh`).
- kaizen_search_sessions — BM25 event search over current workspace. Supports since, agent, kind, limit.
- kaizen_insights — Activity dashboard (7d). kaizen_retro — weekly bets. kaizen_exp_* — experiments.
- kaizen_query / kaizen_cases_* / kaizen_rules_* / kaizen_alerts_check / kaizen_review_* — local trace-to-case automation loop.
- List/summary/insights/metrics/retro are cache-first; set refresh=true to force a full transcript rescan (matches CLI --refresh).
- sessions_list/summary/insights/metrics also accept all_workspaces=true to aggregate across registered workspace-local DBs.
- kaizen_ingest_hook — same as `kaizen ingest hook` (rare; hooks call this).
- kaizen_init — idempotent .kaizen/ + hook patches. kaizen_sync_* — outbox. kaizen_tui — not available (returns JSON stub).

Docs: https://github.com/marquesds/kaizen/blob/main/docs/mcp.md
"#;

fn ok_str(s: String) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(s)]))
}

fn err_str(msg: String) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::error(vec![Content::text(msg)]))
}

async fn run_blocking<T, F>(f: F) -> Result<T, ErrorData>
where
    F: FnOnce() -> Result<T, anyhow::Error> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?
        .map_err(|e: anyhow::Error| ErrorData::internal_error(format!("{e:#}"), None))
}

fn opt_path(ws: &Option<String>) -> Option<std::path::PathBuf> {
    ws.as_ref().map(std::path::PathBuf::from)
}

fn resolve_ws(ws: &WorkspaceArg) -> Result<Option<std::path::PathBuf>, ErrorData> {
    match (ws.workspace.as_deref(), ws.project.as_deref()) {
        (None, None) => Ok(None),
        (w, p) => cli::resolve_target(w.map(std::path::Path::new), p)
            .map(|(path, _)| Some(path))
            .map_err(|e| ErrorData::internal_error(e.to_string(), None)),
    }
}

/// Shared workspace argument for tools.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct WorkspaceArg {
    /// Workspace root (repository path). If omitted, uses the process current directory.
    workspace: Option<String>,
    /// Project name shorthand (mutually exclusive with workspace).
    project: Option<String>,
}

/// Workspace + optional machine-readable JSON (matches CLI `--json` on list/summary).
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct WorkspaceJsonArg {
    /// Workspace root (repository path). If omitted, uses the process current directory.
    workspace: Option<String>,
    /// When true, read from every registered workspace on this machine.
    #[serde(default)]
    all_workspaces: bool,
    /// When true, return the same pretty JSON as `kaizen sessions list --json` or `kaizen summary --json`.
    #[serde(default)]
    json: bool,
    /// When true, run a full agent transcript rescan (matches `kaizen ... --refresh`).
    #[serde(default)]
    refresh: bool,
    /// Cap sessions returned (newest first); only `kaizen_sessions_list` uses this.
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct IngestHookArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    /// `cursor` or `claude`
    source: String,
    /// Same JSON a hook would send on stdin
    payload: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SessionIdArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetSpanTreeArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    id: String,
    #[serde(default)]
    json: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SearchSessionsArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    query: String,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default = "default_search_limit")]
    limit: usize,
}

fn default_search_limit() -> usize {
    50
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MetricsArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default)]
    all_workspaces: bool,
    #[serde(default = "default_days")]
    days: u32,
    /// When true, return pretty JSON
    json: bool,
    #[serde(default)]
    force: bool,
    /// When true, run a full agent transcript rescan (matches `kaizen metrics --refresh`).
    #[serde(default)]
    refresh: bool,
}

fn default_days() -> u32 {
    7
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct MetricsIndexArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SyncRunArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    /// For MCP, must be `true` (single flush). Continuous daemon is not supported here.
    #[serde(default = "default_once_true")]
    once: bool,
}

fn default_once_true() -> bool {
    true
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RetroArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default = "default_days")]
    days: u32,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    json: bool,
    #[serde(default)]
    force: bool,
    /// When true, run a full agent transcript rescan (matches `kaizen retro --refresh`).
    #[serde(default)]
    refresh: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct InsightsArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default)]
    all_workspaces: bool,
    /// When true, run a full agent transcript rescan (matches `kaizen insights --refresh`).
    #[serde(default)]
    refresh: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExpNewArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    name: String,
    hypothesis: String,
    change: String,
    metric: String,
    #[serde(default = "default_bind")]
    bind: String,
    #[serde(default = "default_duration")]
    duration_days: u32,
    #[serde(default = "default_target")]
    target_pct: f64,
    control_commit: Option<String>,
    treatment_commit: Option<String>,
    control_branch: Option<String>,
    treatment_branch: Option<String>,
    control_fingerprint: Option<String>,
    treatment_fingerprint: Option<String>,
}

fn default_bind() -> String {
    "git".to_string()
}
fn default_duration() -> u32 {
    14
}
fn default_target() -> f64 {
    -10.0
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExpIdArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExpTagArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    id: String,
    session: String,
    /// control | treatment | excluded
    variant: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExpReportArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    id: String,
    #[serde(default)]
    json: bool,
    /// Full transcript rescan before computing the report.
    #[serde(default)]
    refresh: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct AnnotateSessionArg {
    /// Target session id.
    session_id: String,
    /// Score 1..=5.
    #[serde(default)]
    score: Option<u8>,
    /// good | bad | interesting | bug | regression
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(flatten)]
    ws: WorkspaceArg,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct QueryToolArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    expr: String,
    #[serde(default)]
    since: Option<String>,
    #[serde(default = "default_search_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CaseCreateArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    session_id: String,
    reason: String,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SinceArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default)]
    since: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct StatusListArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct IdToolArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RuleCreateArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    name: String,
    filter: String,
    action: String,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RuleRunArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct AlertCheckArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default = "default_alert_days")]
    days: u64,
}

fn default_alert_days() -> u64 {
    7
}

#[derive(Clone, Debug)]
pub struct KaizenMcp;

#[tool_router]
impl KaizenMcp {
    #[tool(
        name = "kaizen_capabilities",
        description = "Read first: when to use summary vs metrics, sessions, retro, and other tools. No DB access; static help text only."
    )]
    async fn kaizen_capabilities(
        &self,
        Parameters(_): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        ok_str(MCP_CAPABILITIES.to_string())
    }

    #[tool(
        name = "kaizen_ingest_hook",
        description = "Ingest a hook event (same as `kaizen ingest hook`). Pass payload JSON, not stdin."
    )]
    async fn kaizen_ingest_hook(
        &self,
        Parameters(IngestHookArg {
            ws,
            source,
            payload,
        }): Parameters<IngestHookArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let src = IngestSource::parse(&source)
            .ok_or_else(|| ErrorData::invalid_params("source must be cursor or claude", None))?;
        let w = resolve_ws(&ws)?;
        run_blocking(move || ingest_hook_string(src, &payload, w)).await?;
        ok_str(String::new())
    }

    #[tool(
        name = "kaizen_sessions_list",
        description = "List agent sessions in the workspace. Set json=true for structured output. Optional limit caps rows after sort (newest first). Use refresh=true for a full transcript rescan."
    )]
    async fn kaizen_sessions_list(
        &self,
        Parameters(WorkspaceJsonArg {
            workspace,
            all_workspaces,
            json,
            refresh,
            limit,
        }): Parameters<WorkspaceJsonArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
        let lim = limit.map(|n| n as usize);
        let t = run_blocking(move || {
            cli::sessions_list_text(w.as_deref(), json, refresh, all_workspaces, lim)
        })
        .await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_session_show",
        description = "Show one session (kaizen sessions show)"
    )]
    async fn kaizen_session_show(
        &self,
        Parameters(SessionIdArg { ws, id }): Parameters<SessionIdArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || cli::session_show_text(&id, w.as_deref())).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_search_sessions",
        description = "BM25 full-text search over session events. Args match `kaizen sessions search`: query, since, agent, kind, limit, workspace."
    )]
    async fn search_sessions(
        &self,
        Parameters(SearchSessionsArg {
            ws,
            query,
            since,
            agent,
            kind,
            limit,
        }): Parameters<SearchSessionsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        if crate::core_loop::query::is_structured(&query) {
            let value =
                run_blocking(move || query_value(w, &query, since.as_deref(), limit)).await?;
            return Ok(CallToolResult::structured(value));
        }
        let (hits, fallback) = run_blocking(move || {
            crate::shell::search::sessions_search_hits(
                w.as_deref(),
                &query,
                since.as_deref(),
                agent.as_deref(),
                kind.as_deref(),
                limit,
            )
        })
        .await?;
        Ok(CallToolResult::structured(serde_json::json!({
            "fallback": fallback,
            "count": hits.len(),
            "hits": hits,
        })))
    }

    #[tool(
        name = "kaizen_query",
        description = "Structured trace query. Args: expr, since, limit, workspace."
    )]
    async fn kaizen_query(
        &self,
        Parameters(QueryToolArg {
            ws,
            expr,
            since,
            limit,
        }): Parameters<QueryToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || query_value(w, &expr, since.as_deref(), limit)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(
        name = "kaizen_cases_mine",
        description = "Mine cases from low evals and bad feedback."
    )]
    async fn kaizen_cases_mine(
        &self,
        Parameters(SinceArg { ws, since }): Parameters<SinceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || cases_mine_value(w, since.as_deref())).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(
        name = "kaizen_cases_create",
        description = "Create one case for a session."
    )]
    async fn kaizen_cases_create(
        &self,
        Parameters(CaseCreateArg {
            ws,
            session_id,
            reason,
            label,
        }): Parameters<CaseCreateArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value =
            run_blocking(move || cases_create_value(w, &session_id, &reason, label)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(name = "kaizen_cases_list", description = "List local cases.")]
    async fn kaizen_cases_list(
        &self,
        Parameters(StatusListArg { ws, status }): Parameters<StatusListArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || cases_list_value(w, status.as_deref())).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(name = "kaizen_cases_show", description = "Show one local case.")]
    async fn kaizen_cases_show(
        &self,
        Parameters(IdToolArg { ws, id }): Parameters<IdToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || case_show_value(w, &id)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(name = "kaizen_cases_archive", description = "Archive one local case.")]
    async fn kaizen_cases_archive(
        &self,
        Parameters(IdToolArg { ws, id }): Parameters<IdToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        run_blocking(move || case_archive_value(w, &id)).await?;
        ok_str("archived".into())
    }

    #[tool(
        name = "kaizen_rules_create",
        description = "Create local rule: action=create_case|queue_review|emit_alert."
    )]
    async fn kaizen_rules_create(
        &self,
        Parameters(RuleCreateArg {
            ws,
            name,
            filter,
            action,
            message,
        }): Parameters<RuleCreateArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value =
            run_blocking(move || rule_create_value(w, &name, &filter, &action, message)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(name = "kaizen_rules_list", description = "List local rules.")]
    async fn kaizen_rules_list(
        &self,
        Parameters(ws): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || rules_list_value(w)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(name = "kaizen_rules_run", description = "Run enabled local rules.")]
    async fn kaizen_rules_run(
        &self,
        Parameters(RuleRunArg { ws, since, dry_run }): Parameters<RuleRunArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || rules_run_value(w, since.as_deref(), dry_run)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(name = "kaizen_rules_enable", description = "Enable one local rule.")]
    async fn kaizen_rules_enable(
        &self,
        Parameters(IdToolArg { ws, id }): Parameters<IdToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        run_blocking(move || rule_enable_value(w, &id, true)).await?;
        ok_str("enabled".into())
    }

    #[tool(name = "kaizen_rules_disable", description = "Disable one local rule.")]
    async fn kaizen_rules_disable(
        &self,
        Parameters(IdToolArg { ws, id }): Parameters<IdToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        run_blocking(move || rule_enable_value(w, &id, false)).await?;
        ok_str("disabled".into())
    }

    #[tool(
        name = "kaizen_alerts_check",
        description = "Run built-in local alert checks."
    )]
    async fn kaizen_alerts_check(
        &self,
        Parameters(AlertCheckArg { ws, days }): Parameters<AlertCheckArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || alerts_value(w, days)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(
        name = "kaizen_review_list",
        description = "List local review queue items."
    )]
    async fn kaizen_review_list(
        &self,
        Parameters(StatusListArg { ws, status }): Parameters<StatusListArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || review_list_value(w, status.as_deref())).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(name = "kaizen_review_show", description = "Show one review item.")]
    async fn kaizen_review_show(
        &self,
        Parameters(IdToolArg { ws, id }): Parameters<IdToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let value = run_blocking(move || review_show_value(w, &id)).await?;
        Ok(CallToolResult::structured(value))
    }

    #[tool(
        name = "kaizen_review_resolve",
        description = "Resolve one review item."
    )]
    async fn kaizen_review_resolve(
        &self,
        Parameters(IdToolArg { ws, id }): Parameters<IdToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        run_blocking(move || review_status_value(w, &id, crate::core_loop::ReviewStatus::Resolved))
            .await?;
        ok_str("resolved".into())
    }

    #[tool(
        name = "kaizen_review_dismiss",
        description = "Dismiss one review item."
    )]
    async fn kaizen_review_dismiss(
        &self,
        Parameters(IdToolArg { ws, id }): Parameters<IdToolArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        run_blocking(move || {
            review_status_value(w, &id, crate::core_loop::ReviewStatus::Dismissed)
        })
        .await?;
        ok_str("dismissed".into())
    }

    #[tool(
        name = "kaizen_summary",
        description = "Roll up session counts, USD cost, top tools, by-agent/model. For **code** hotspots and slow tool p95, use `kaizen_metrics` instead. Set json=true to match `kaizen summary --json` (optional `cost_note` when sessions exist but stored cost rollup is zero)."
    )]
    async fn kaizen_summary(
        &self,
        Parameters(WorkspaceJsonArg {
            workspace,
            all_workspaces,
            json,
            refresh,
            limit: _,
        }): Parameters<WorkspaceJsonArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
        let t = run_blocking(move || {
            cli::summary_text(
                w.as_deref(),
                json,
                refresh,
                all_workspaces,
                DataSource::Local,
            )
        })
        .await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_tui",
        description = "Interactive TUI is not available via MCP. Returns guidance."
    )]
    async fn kaizen_tui(
        &self,
        Parameters(_): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::structured_error(serde_json::json!({
            "available": false,
            "reason": "interactive",
            "cli": "kaizen tui [ --workspace <path> ]"
        })))
    }

    #[tool(
        name = "kaizen_init",
        description = "Idempotent workspace setup (kaizen init)"
    )]
    async fn kaizen_init(
        &self,
        Parameters(ws): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || init::init_text(w.as_deref())).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_insights",
        description = "Session insights (kaizen insights)"
    )]
    async fn kaizen_insights(
        &self,
        Parameters(InsightsArg {
            ws,
            all_workspaces,
            refresh,
        }): Parameters<InsightsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || {
            insights::insights_text(w.as_deref(), all_workspaces, refresh, DataSource::Local)
        })
        .await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_metrics",
        description = "Repo + tool intelligence: hottest files, slow tools (p95), token/reasoning sinks, agent pain. Not for simple cost rollups — use `kaizen_summary` first."
    )]
    async fn kaizen_metrics(
        &self,
        Parameters(MetricsArg {
            ws,
            all_workspaces,
            days,
            json,
            force,
            refresh,
        }): Parameters<MetricsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || {
            metrics::metrics_text(
                w.as_deref(),
                days,
                json,
                force,
                all_workspaces,
                refresh,
                DataSource::Local,
            )
        })
        .await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_metrics_index",
        description = "Rebuild repo snapshot index (kaizen metrics index)"
    )]
    async fn kaizen_metrics_index(
        &self,
        Parameters(MetricsIndexArg { ws, force }): Parameters<MetricsIndexArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || metrics::metrics_index_text(w.as_deref(), force)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_sync_run",
        description = "Flush outbox (kaizen sync run). Use once=true (default). Continuous mode is not supported."
    )]
    async fn kaizen_sync_run(
        &self,
        Parameters(SyncRunArg { ws, once }): Parameters<SyncRunArg>,
    ) -> Result<CallToolResult, ErrorData> {
        if !once {
            return err_str(
                "once=false (continuous sync daemon) is not supported over MCP. Run `kaizen sync run` in a shell, or pass once=true (default).".into(),
            );
        }
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || sync::sync_run_text(w.as_deref(), true)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_sync_status",
        description = "Outbox and sync health (kaizen sync status)"
    )]
    async fn kaizen_sync_status(
        &self,
        Parameters(ws): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || sync::sync_status_text(w.as_deref())).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_new",
        description = "Create experiment (kaizen exp new)"
    )]
    async fn kaizen_exp_new(
        &self,
        Parameters(ExpNewArg {
            ws,
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
            control_fingerprint,
            treatment_fingerprint,
        }): Parameters<ExpNewArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let args = NewArgs {
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
            control_fingerprint,
            treatment_fingerprint,
        };
        let t = run_blocking(move || exp::exp_new_text(w.as_deref(), args)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_list",
        description = "List experiments (kaizen exp list)"
    )]
    async fn kaizen_exp_list(
        &self,
        Parameters(ws): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || exp::exp_list_text(w.as_deref())).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_status",
        description = "Show experiment (kaizen exp status)"
    )]
    async fn kaizen_exp_status(
        &self,
        Parameters(ExpIdArg { ws, id }): Parameters<ExpIdArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || exp::exp_status_text(w.as_deref(), &id)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_tag",
        description = "Tag session variant (kaizen exp tag)"
    )]
    async fn kaizen_exp_tag(
        &self,
        Parameters(ExpTagArg {
            ws,
            id,
            session,
            variant,
        }): Parameters<ExpTagArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t =
            run_blocking(move || exp::exp_tag_text(w.as_deref(), &id, &session, &variant)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_report",
        description = "Experiment report (kaizen exp report). Optional refresh: true forces a full transcript rescan before computing the report."
    )]
    async fn kaizen_exp_report(
        &self,
        Parameters(ExpReportArg {
            ws,
            id,
            json,
            refresh,
        }): Parameters<ExpReportArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t =
            run_blocking(move || exp::exp_report_text(w.as_deref(), &id, json, refresh)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_conclude",
        description = "Conclude experiment (kaizen exp conclude)"
    )]
    async fn kaizen_exp_conclude(
        &self,
        Parameters(ExpIdArg { ws, id }): Parameters<ExpIdArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || exp::exp_conclude_text(w.as_deref(), &id)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_start",
        description = "Start experiment — transition Draft → Running (kaizen exp start)"
    )]
    async fn kaizen_exp_start(
        &self,
        Parameters(ExpIdArg { ws, id }): Parameters<ExpIdArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || exp::exp_start_text(w.as_deref(), &id)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_archive",
        description = "Archive experiment — transition Concluded → Archived (kaizen exp archive)"
    )]
    async fn kaizen_exp_archive(
        &self,
        Parameters(ExpIdArg { ws, id }): Parameters<ExpIdArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || exp::exp_archive_text(w.as_deref(), &id)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_retro",
        description = "Heuristic retro report (kaizen retro). Prefer json=true for machine parsing."
    )]
    async fn kaizen_retro(
        &self,
        Parameters(RetroArg {
            ws,
            days,
            dry_run,
            json,
            force,
            refresh,
        }): Parameters<RetroArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || {
            retro::retro_stdout(
                w.as_deref(),
                days,
                dry_run,
                json,
                force,
                refresh,
                DataSource::Local,
            )
        })
        .await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_annotate_session",
        description = "Attach human feedback (score 1-5, label, free-text note) to a session."
    )]
    async fn kaizen_annotate_session(
        &self,
        Parameters(AnnotateSessionArg {
            session_id,
            score,
            label,
            note,
            ws,
        }): Parameters<AnnotateSessionArg>,
    ) -> Result<CallToolResult, ErrorData> {
        use crate::feedback::types::FeedbackLabel;
        let parsed_label = match label.as_deref() {
            Some(s) => {
                let l = FeedbackLabel::from_str_opt(s);
                if l.is_none() {
                    return Err(ErrorData::invalid_params(
                        format!("unknown label: {s}"),
                        None,
                    ));
                }
                l
            }
            None => None,
        };
        let w = resolve_ws(&ws)?;
        run_blocking(move || {
            crate::shell::feedback::cmd_sessions_annotate(
                &session_id,
                score,
                parsed_label,
                note,
                w.as_deref(),
            )
        })
        .await?;
        ok_str("annotated".into())
    }

    #[tool(
        name = "get_session_span_tree",
        description = "Return the nested tool-span tree for a session. Each node carries tool name, status, subtree cost, depth, and children. Use json=true for structured output."
    )]
    async fn get_session_span_tree(
        &self,
        Parameters(GetSpanTreeArg { ws, id, json }): Parameters<GetSpanTreeArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = resolve_ws(&ws)?;
        let t = run_blocking(move || {
            crate::shell::cli::cmd_sessions_tree_text(&id, 999, json, w.as_deref())
        })
        .await?;
        ok_str(t)
    }
}

fn workspace_store(
    w: Option<std::path::PathBuf>,
) -> anyhow::Result<(std::path::PathBuf, crate::store::Store)> {
    let ws = crate::shell::cli::workspace_path(w.as_deref())?;
    let store = crate::store::Store::open(&crate::core::workspace::db_path(&ws)?)?;
    Ok((ws, store))
}

fn workspace_read_store(
    w: Option<std::path::PathBuf>,
) -> anyhow::Result<(std::path::PathBuf, crate::store::Store)> {
    let ws = crate::shell::cli::workspace_path(w.as_deref())?;
    let store = crate::shell::cli::open_workspace_read_store(&ws, false)?;
    Ok((ws, store))
}

fn query_value(
    w: Option<std::path::PathBuf>,
    expr: &str,
    since: Option<&str>,
    limit: usize,
) -> anyhow::Result<serde_json::Value> {
    let (ws, store) = workspace_read_store(w)?;
    let start = crate::core_loop::time::parse_window(since, 7)?;
    let hits = crate::core_loop::query::run(&store, &ws.to_string_lossy(), expr, start, limit)?;
    Ok(serde_json::json!({ "count": hits.len(), "hits": hits }))
}

fn cases_mine_value(
    w: Option<std::path::PathBuf>,
    since: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let (_, store) = workspace_store(w)?;
    let rows = crate::core_loop::cases::mine(
        &store,
        crate::core_loop::time::parse_window(since, 7)?,
        crate::core_loop::time::now_ms(),
    )?;
    Ok(serde_json::json!({ "count": rows.len(), "cases": rows }))
}

fn cases_create_value(
    w: Option<std::path::PathBuf>,
    session_id: &str,
    reason: &str,
    label: Option<String>,
) -> anyhow::Result<serde_json::Value> {
    let (_, store) = workspace_store(w)?;
    let session = store
        .get_session(session_id)?
        .ok_or_else(|| anyhow::anyhow!("session not found"))?;
    let key = format!("manual:{session_id}:{reason}");
    let row = crate::core_loop::cases::create_case(
        &store,
        &session,
        &key,
        reason,
        label,
        crate::core_loop::time::now_ms(),
    )?;
    Ok(serde_json::json!({ "case": row }))
}

fn cases_list_value(
    w: Option<std::path::PathBuf>,
    status: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let (_, store) = workspace_read_store(w)?;
    let rows = crate::core_loop::cases::list(&store, case_status(status))?;
    Ok(serde_json::json!({ "count": rows.len(), "cases": rows }))
}

fn case_show_value(w: Option<std::path::PathBuf>, id: &str) -> anyhow::Result<serde_json::Value> {
    let (_, store) = workspace_read_store(w)?;
    let row = crate::core_loop::cases::get(&store, id)?;
    let refs = crate::core_loop::cases::refs(&store, id)?;
    Ok(serde_json::json!({ "case": row, "refs": refs }))
}

fn case_archive_value(w: Option<std::path::PathBuf>, id: &str) -> anyhow::Result<()> {
    crate::core_loop::cases::archive(&workspace_store(w)?.1, id)
}

fn rule_create_value(
    w: Option<std::path::PathBuf>,
    name: &str,
    filter: &str,
    action: &str,
    message: Option<String>,
) -> anyhow::Result<serde_json::Value> {
    let (_, store) = workspace_store(w)?;
    let rule = crate::core_loop::rules::create(
        &store,
        name,
        filter,
        rule_action(action, message)?,
        crate::core_loop::time::now_ms(),
    )?;
    Ok(serde_json::json!({ "rule": rule }))
}

fn rules_list_value(w: Option<std::path::PathBuf>) -> anyhow::Result<serde_json::Value> {
    let rows = crate::core_loop::rules::list(&workspace_read_store(w)?.1)?;
    Ok(serde_json::json!({ "count": rows.len(), "rules": rows }))
}

fn rules_run_value(
    w: Option<std::path::PathBuf>,
    since: Option<&str>,
    dry_run: bool,
) -> anyhow::Result<serde_json::Value> {
    let (ws, store) = workspace_store(w)?;
    let start = crate::core_loop::time::parse_window(since, 7)?;
    let rows = crate::core_loop::rules::run_enabled(
        &store,
        &ws.to_string_lossy(),
        start,
        crate::core_loop::time::now_ms(),
        dry_run,
    )?;
    Ok(serde_json::json!({ "count": rows.len(), "runs": rows }))
}

fn rule_enable_value(w: Option<std::path::PathBuf>, id: &str, enabled: bool) -> anyhow::Result<()> {
    crate::core_loop::rules::set_enabled(&workspace_store(w)?.1, id, enabled)
}

fn alerts_value(w: Option<std::path::PathBuf>, days: u64) -> anyhow::Result<serde_json::Value> {
    let (ws, store) = workspace_store(w)?;
    let rows = crate::core_loop::alerts::check_builtin(
        &store,
        &ws.to_string_lossy(),
        crate::core_loop::time::since_days(days),
        crate::core_loop::time::now_ms(),
    )?;
    Ok(serde_json::json!({ "count": rows.len(), "alerts": rows }))
}

fn review_list_value(
    w: Option<std::path::PathBuf>,
    status: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let rows = crate::core_loop::review::list(&workspace_read_store(w)?.1, review_status(status))?;
    Ok(serde_json::json!({ "count": rows.len(), "items": rows }))
}

fn review_show_value(w: Option<std::path::PathBuf>, id: &str) -> anyhow::Result<serde_json::Value> {
    let row = crate::core_loop::review::get(&workspace_read_store(w)?.1, id)?;
    Ok(serde_json::json!({ "item": row }))
}

fn review_status_value(
    w: Option<std::path::PathBuf>,
    id: &str,
    status: crate::core_loop::ReviewStatus,
) -> anyhow::Result<()> {
    crate::core_loop::review::set_status(
        &workspace_store(w)?.1,
        id,
        status,
        crate::core_loop::time::now_ms(),
    )
}

fn case_status(raw: Option<&str>) -> Option<crate::core_loop::CaseStatus> {
    raw.map(|s| {
        if s == "archived" {
            crate::core_loop::CaseStatus::Archived
        } else {
            crate::core_loop::CaseStatus::Open
        }
    })
}

fn review_status(raw: Option<&str>) -> Option<crate::core_loop::ReviewStatus> {
    raw.map(|s| match s {
        "resolved" => crate::core_loop::ReviewStatus::Resolved,
        "dismissed" => crate::core_loop::ReviewStatus::Dismissed,
        _ => crate::core_loop::ReviewStatus::Open,
    })
}

fn rule_action(raw: &str, message: Option<String>) -> anyhow::Result<crate::core_loop::RuleAction> {
    Ok(match raw {
        "create_case" => crate::core_loop::RuleAction::CreateCase { label: message },
        "queue_review" => crate::core_loop::RuleAction::QueueReview { title: message },
        "emit_alert" => crate::core_loop::RuleAction::EmitAlert {
            severity: crate::core_loop::AlertSeverity::Warning,
        },
        _ => anyhow::bail!("unknown rule action"),
    })
}

#[tool_handler(
    name = "kaizen",
    version = "0.1.0",
    instructions = "kaizen: local agent telemetry. Call `kaizen_capabilities` first if unsure. Cost/volume: `kaizen_summary`. Code hotspots and slow tools: `kaizen_metrics`. Most CLI workflows are here; shell-only: doctor, guidance, gc, completions, proxy, telemetry. Workspace defaults to the server cwd. `kaizen_tui` is interactive CLI-only. `kaizen_sync_run` supports once=true only."
)]
impl ServerHandler for KaizenMcp {}
