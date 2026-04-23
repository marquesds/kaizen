// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP `#[tool]` handlers (stdio server).

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
struct MetricsArg {
    #[serde(flatten)]
    ws: WorkspaceArg,
    #[serde(default = "default_days")]
    days: u32,
    /// When true, return pretty JSON
    json: bool,
    #[serde(default)]
    force: bool,
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

#[derive(Clone, Debug)]
pub struct KaizenMcp;

#[tool_router]
impl KaizenMcp {
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
        description = "List sessions (kaizen sessions list)"
    )]
    async fn kaizen_sessions_list(
        &self,
        Parameters(WorkspaceArg { workspace }): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
        let t = run_blocking(move || cli::sessions_list_text(w.as_deref())).await?;
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
        name = "kaizen_summary",
        description = "Aggregate session + cost stats (kaizen summary)"
    )]
    async fn kaizen_summary(
        &self,
        Parameters(WorkspaceArg { workspace }): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
        let t = run_blocking(move || cli::summary_text(w.as_deref())).await?;
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
        Parameters(WorkspaceArg { workspace }): Parameters<WorkspaceArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&workspace);
        let t = run_blocking(move || insights::insights_text(w.as_deref())).await?;
        ok_str(t)
    }

    #[tool(
        name = "kaizen_metrics",
        description = "Smart metrics (kaizen metrics)"
    )]
    async fn kaizen_metrics(
        &self,
        Parameters(MetricsArg {
            ws,
            days,
            json,
            force,
        }): Parameters<MetricsArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&ws.workspace);
        let t =
            run_blocking(move || metrics::metrics_text(w.as_deref(), days, json, force)).await?;
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
        }): Parameters<RetroArg>,
    ) -> Result<CallToolResult, ErrorData> {
        let w = opt_path(&ws.workspace);
        let t = run_blocking(move || retro::retro_stdout(w.as_deref(), days, dry_run, json, force))
            .await?;
        ok_str(t)
    }
}

#[tool_handler(
    name = "kaizen",
    version = "0.1.0",
    instructions = "kaizen: local agent telemetry. Tools mirror the `kaizen` CLI (see `kaizen --help`). Workspace defaults to the server process cwd if omitted. Use `kaizen_tui` for why the terminal UI is CLI-only. Use `kaizen_sync_run` with once=true only."
)]
impl ServerHandler for KaizenMcp {}
