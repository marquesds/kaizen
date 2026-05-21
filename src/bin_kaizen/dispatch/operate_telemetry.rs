use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;
use std::path::PathBuf;

pub(super) fn telemetry(cmd: TelemetrySubcommand) -> anyhow::Result<()> {
    match cmd {
        TelemetrySubcommand::Init {
            workspace,
            project,
            exporter_type,
            path,
        } => telemetry_configure(
            workspace,
            project,
            default_telemetry_options(exporter_type, path),
        ),
        TelemetrySubcommand::Doctor { workspace, project } => telemetry_doctor(workspace, project),
        TelemetrySubcommand::Pull {
            days,
            workspace,
            project,
        } => telemetry_pull(days, workspace, project),
        TelemetrySubcommand::Push {
            days,
            workspace,
            project,
            all_workspaces,
            dry_run,
        } => telemetry_push(days, workspace, project, all_workspaces, dry_run),
        TelemetrySubcommand::PrintSchema => kaizen::shell::telemetry::cmd_telemetry_print_schema(),
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
            let opts = kaizen::shell::telemetry::ConfigureOptions {
                exporter_type: exporter_type.map(|t| t.as_str().to_string()),
                path,
                api_key,
                site,
                host,
                endpoint,
                non_interactive,
            };
            telemetry_configure(workspace, project, opts)
        }
        TelemetrySubcommand::Test { workspace, project } => telemetry_test(workspace, project),
        TelemetrySubcommand::PrintEffectiveConfig { workspace, project } => {
            telemetry_effective(workspace, project)
        }
        TelemetrySubcommand::Tail {
            workspace,
            project,
            file,
            no_follow,
            json,
        } => telemetry_tail(workspace, project, file, no_follow, json),
    }
}

fn default_telemetry_options(
    exporter_type: Option<TelemetryExporterKind>,
    path: Option<PathBuf>,
) -> kaizen::shell::telemetry::ConfigureOptions {
    kaizen::shell::telemetry::ConfigureOptions {
        exporter_type: exporter_type.map(|t| t.as_str().to_string()),
        path,
        ..Default::default()
    }
}

fn telemetry_configure(
    workspace: Option<PathBuf>,
    project: Option<String>,
    options: kaizen::shell::telemetry::ConfigureOptions,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::telemetry::cmd_telemetry_configure(ws.as_deref(), options)
}

fn telemetry_doctor(workspace: Option<PathBuf>, project: Option<String>) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::telemetry::cmd_telemetry_doctor(ws.as_deref())
}

fn telemetry_pull(
    days: u32,
    workspace: Option<PathBuf>,
    project: Option<String>,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::telemetry::cmd_telemetry_pull(ws.as_deref(), days)
}

fn telemetry_push(
    days: u32,
    workspace: Option<PathBuf>,
    project: Option<String>,
    all_workspaces: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::telemetry::cmd_telemetry_push(ws.as_deref(), all_workspaces, days, dry_run)
}

fn telemetry_test(workspace: Option<PathBuf>, project: Option<String>) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::telemetry::cmd_telemetry_test(ws.as_deref())
}

fn telemetry_effective(workspace: Option<PathBuf>, project: Option<String>) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::telemetry::cmd_telemetry_print_effective(ws.as_deref())
}

fn telemetry_tail(
    workspace: Option<PathBuf>,
    project: Option<String>,
    file: Option<PathBuf>,
    no_follow: bool,
    json: bool,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::telemetry_tail::cmd_telemetry_tail(ws.as_deref(), file, no_follow, json)
}
