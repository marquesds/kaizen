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
    subcmd: Option<GuidanceCommand>,
    days: u32,
    json: bool,
    workspace: Option<PathBuf>,
    project: Option<String>,
    refresh: bool,
    source: DataSource,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    if let Some(cmd) = subcmd {
        return guidance_subcmd(ws, cmd);
    }
    kaizen::shell::guidance::cmd_guidance(ws.as_deref(), days, json, refresh, source)
}

fn guidance_subcmd(ws: Option<PathBuf>, cmd: GuidanceCommand) -> anyhow::Result<()> {
    match cmd {
        GuidanceCommand::Score {
            days,
            min_sessions,
            json,
            ws: f,
        } => {
            let ws = resolve_ws(
                f.workspace.as_deref().or(ws.as_deref()),
                f.project.as_deref(),
            )?;
            kaizen::shell::guidance_science::cmd_score(ws.as_deref(), days, min_sessions, json)
        }
        GuidanceCommand::Propose {
            artifact,
            max_ops,
            llm,
            json,
            ws: f,
        } => {
            let ws = resolve_ws(
                f.workspace.as_deref().or(ws.as_deref()),
                f.project.as_deref(),
            )?;
            kaizen::shell::guidance_science::cmd_propose(
                ws.as_deref(),
                &artifact,
                max_ops,
                llm,
                json,
            )
        }
        GuidanceCommand::Candidates { subcmd } => guidance_candidates(ws, subcmd),
    }
}

fn guidance_candidates(
    parent_ws: Option<PathBuf>,
    cmd: GuidanceCandidatesCommand,
) -> anyhow::Result<()> {
    use kaizen::guidance::CandidateStatus;
    use kaizen::shell::guidance_candidates::CandidateOp;
    match cmd {
        GuidanceCandidatesCommand::List { json, ws } => {
            let ws = resolve_ws(
                ws.workspace.as_deref().or(parent_ws.as_deref()),
                ws.project.as_deref(),
            )?;
            kaizen::shell::guidance_candidates::cmd(ws.as_deref(), CandidateOp::List { json })
        }
        GuidanceCandidatesCommand::Show { id, json, ws } => {
            let ws = resolve_ws(
                ws.workspace.as_deref().or(parent_ws.as_deref()),
                ws.project.as_deref(),
            )?;
            kaizen::shell::guidance_candidates::cmd(ws.as_deref(), CandidateOp::Show { id, json })
        }
        GuidanceCandidatesCommand::Reject(a) => {
            candidate_set(parent_ws, a, CandidateStatus::Rejected)
        }
        GuidanceCandidatesCommand::Archive(a) => {
            candidate_set(parent_ws, a, CandidateStatus::Archived)
        }
    }
}

fn candidate_set(
    parent_ws: Option<PathBuf>,
    args: IdOnly,
    status: kaizen::guidance::CandidateStatus,
) -> anyhow::Result<()> {
    let ws = resolve_ws(
        args.ws.workspace.as_deref().or(parent_ws.as_deref()),
        args.ws.project.as_deref(),
    )?;
    let op = kaizen::shell::guidance_candidates::CandidateOp::Set {
        id: args.id,
        status,
    };
    kaizen::shell::guidance_candidates::cmd(ws.as_deref(), op)
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
    let _ = cmd.subcmd;
    kaizen::shell::projects::cmd_projects_list(cmd.json, cmd.include_missing)
}
