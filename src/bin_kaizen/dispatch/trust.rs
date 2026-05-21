use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;
use kaizen::DataSource;
use std::path::PathBuf;

pub(super) fn summary(
    workspace: Option<PathBuf>,
    project: Option<String>,
    all_workspaces: bool,
    json: bool,
    refresh: bool,
    source: DataSource,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::cli::cmd_summary(ws.as_deref(), json, refresh, all_workspaces, source)
}

pub(super) fn tui(workspace: Option<PathBuf>, project: Option<String>) -> anyhow::Result<()> {
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

pub(super) fn init(
    workspace: Option<PathBuf>,
    project: Option<String>,
    deep: bool,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::cli::cmd_init(ws.as_deref(), deep)
}

pub(super) fn doctor(workspace: Option<PathBuf>, project: Option<String>) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    let code = kaizen::shell::doctor::cmd_doctor(ws.as_deref())?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

pub(super) fn load(
    workspace: Option<PathBuf>,
    project: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::load::cmd_load(ws.as_deref(), json)
}

pub(super) fn insights(
    workspace: Option<PathBuf>,
    project: Option<String>,
    all_workspaces: bool,
    refresh: bool,
    source: DataSource,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::insights::cmd_insights(ws.as_deref(), all_workspaces, refresh, source)
}

pub(super) fn guidance(
    days: u32,
    json: bool,
    workspace: Option<PathBuf>,
    project: Option<String>,
    refresh: bool,
    source: DataSource,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::guidance::cmd_guidance(ws.as_deref(), days, json, refresh, source)
}

pub(super) fn observe(
    agent: String,
    workspace: Option<PathBuf>,
    project: Option<String>,
    command: Vec<String>,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::observe::cmd_observe(ws.as_deref(), &agent, &command)
}

pub(super) fn projects(cmd: ProjectsCommand) -> anyhow::Result<()> {
    match cmd {
        ProjectsCommand::List => kaizen::shell::projects::cmd_projects_list(),
    }
}
