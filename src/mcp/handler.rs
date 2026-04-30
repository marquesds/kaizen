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
- kaizen_sessions_list / kaizen_session_show — Session list and one session metadata. Optional json on list.
- mcp/search_sessions — BM25 event search over current workspace. Supports since, agent, kind, limit.
- kaizen_insights — Activity dashboard (7d). kaizen_retro — weekly bets. kaizen_exp_* — experiments.
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

/// Shared workspace argument for tools.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct WorkspaceArg {
    /// Workspace root (repository path). If omitted, uses the process current directory.
    workspace: Option<String>,
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
        let w = opt_path(&ws.workspace);
        run_blocking(move || ingest_hook_string(src, &payload, w)).await?;
        ok_str(String::new())
    }

    #[tool(
        name = "kaizen_sessions_list",
        description = "List agent sessions in the workspace. Set json=true for the same array object as `kaizen sessions list --json`. Default text table."
    )]
    async fn kaizen_sessions_list(
        &self,
        Parameters(WorkspaceJsonArg {
            workspace,
            all_workspaces,
            json,
            refresh,
        }): Parameters<WorkspaceJsonArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
        let t = run_blocking(move || {
            cli::sessions_list_text(w.as_deref(), json, refresh, all_workspaces)
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
        let w = opt_path(&ws.workspace);
        let t = run_blocking(move || cli::session_show_text(&id, w.as_deref())).await?;
        ok_str(t)
    }

    #[tool(
        name = "mcp/search_sessions",
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
        let w = opt_path(&ws.workspace);
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
        name = "kaizen_summary",
        description = "Roll up session counts, USD cost, top tools, by-agent/model. For **code** hotspots and slow tool p95, use `kaizen_metrics` instead. Set json=true to match `kaizen summary --json`."
    )]
    async fn kaizen_summary(
        &self,
        Parameters(WorkspaceJsonArg {
            workspace,
            all_workspaces,
            json,
            refresh,
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
        Parameters(WorkspaceArg { workspace }): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
        let t = run_blocking(move || sync::sync_run_text(w.as_deref(), true)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_sync_status",
        description = "Outbox and sync health (kaizen sync status)"
    )]
    async fn kaizen_sync_status(
        &self,
        Parameters(WorkspaceArg { workspace }): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
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
        }): Parameters<ExpNewArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&ws.workspace);
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
        Parameters(WorkspaceArg { workspace }): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
        let t =
            run_blocking(move || exp::exp_tag_text(w.as_deref(), &id, &session, &variant)).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_exp_report",
        description = "Experiment report (kaizen exp report)"
    )]
    async fn kaizen_exp_report(
        &self,
        Parameters(ExpReportArg { ws, id, json }): Parameters<ExpReportArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&ws.workspace);
        let t = run_blocking(move || exp::exp_report_text(w.as_deref(), &id, json)).await?;
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
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
        let w = opt_path(&ws.workspace);
        let t = run_blocking(move || {
            crate::shell::cli::cmd_sessions_tree_text(&id, 999, json, w.as_deref())
        })
        .await?;
        ok_str(t)
    }
}

#[tool_handler(
    name = "kaizen",
    version = "0.1.0",
    instructions = "kaizen: local agent telemetry. Call `kaizen_capabilities` first if unsure. Cost/volume: `kaizen_summary`. Code hotspots and slow tools: `kaizen_metrics`. Most CLI workflows are here; shell-only: doctor, guidance, gc, completions, proxy, telemetry. Workspace defaults to the server cwd. `kaizen_tui` is interactive CLI-only. `kaizen_sync_run` supports once=true only."
)]
impl ServerHandler for KaizenMcp {}
