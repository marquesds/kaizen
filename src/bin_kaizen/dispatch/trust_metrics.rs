use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;
use std::path::PathBuf;

pub(super) struct MetricsRequest {
    pub(super) subcmd: Option<MetricsCommand>,
    pub(super) report: MetricsReportArgs,
}

pub(super) fn metrics(req: MetricsRequest) -> anyhow::Result<()> {
    let MetricsRequest { subcmd, report } = req;
    match subcmd {
        Some(MetricsCommand::Report(report)) => metrics_report(report),
        Some(MetricsCommand::Index {
            workspace,
            project,
            force,
        }) => metrics_index(workspace, project, force),
        Some(MetricsCommand::Quality {
            workspace,
            project,
            days,
            json,
        }) => metrics_quality(workspace, project, days, json),
        Some(MetricsCommand::Aggregates { subcmd }) => metrics_aggregates(subcmd),
        None => metrics_report(report),
    }
}

fn metrics_index(
    workspace: Option<PathBuf>,
    project: Option<String>,
    force: bool,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::metrics::cmd_metrics_index(ws.as_deref(), force)
}

fn metrics_quality(
    workspace: Option<PathBuf>,
    project: Option<String>,
    days: u32,
    json: bool,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::metrics::cmd_metrics_quality(ws.as_deref(), days, json)
}

fn metrics_aggregates(subcmd: MetricsAggregatesCommand) -> anyhow::Result<()> {
    match subcmd {
        MetricsAggregatesCommand::Rebuild { workspace, project } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::extensions::cmd_aggregates_rebuild(ws.as_deref())
        }
    }
}

fn metrics_report(req: MetricsReportArgs) -> anyhow::Result<()> {
    let ws = resolve_ws(req.workspace.as_deref(), req.project.as_deref())?;
    kaizen::shell::metrics::cmd_metrics(
        ws.as_deref(),
        req.days,
        req.json,
        req.force,
        req.all_workspaces,
        req.refresh,
        req.source,
    )
}
