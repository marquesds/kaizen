use crate::bin_kaizen::args::*;
use crate::bin_kaizen::dispatch::trust::load;
use crate::bin_kaizen::workspace::resolve_ws;
use std::path::PathBuf;

pub(super) fn sessions(cmd: SessionsCommand) -> anyhow::Result<()> {
    match cmd {
        SessionsCommand::List {
            workspace,
            project,
            all_workspaces,
            json,
            limit,
            refresh,
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
        SessionsCommand::Load {
            workspace,
            project,
            json,
        } => load(workspace, project, json),
        SessionsCommand::Show {
            id,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_session_show(&id, ws.as_deref())
        }
        SessionsCommand::Annotate {
            id,
            score,
            label,
            note,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::feedback::cmd_sessions_annotate(&id, score, label, note, ws.as_deref())
        }
        SessionsCommand::Tree {
            id,
            depth,
            json,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_sessions_tree(&id, depth, json, ws.as_deref())
        }
        SessionsCommand::Trace {
            id,
            json,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::cli::cmd_sessions_trace(&id, json, ws.as_deref())
        }
        SessionsCommand::Search {
            query,
            since,
            agent,
            kind,
            limit,
            workspace,
            project,
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
    }
}

pub(super) fn search(cmd: SearchCommand) -> anyhow::Result<()> {
    let SearchCommand::Reindex { workspace, project } = cmd;
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::search::cmd_search_reindex(ws.as_deref())
}

pub(super) fn query(
    expr: String,
    since: Option<String>,
    limit: usize,
    json: bool,
    workspace: Option<PathBuf>,
    project: Option<String>,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::core_query::cmd_query(ws.as_deref(), &expr, since.as_deref(), limit, json)
}

pub(super) fn feedback(cmd: FeedbackCommand) -> anyhow::Result<()> {
    let FeedbackCommand::List {
        workspace,
        project,
        label,
        since,
        json,
    } = cmd;
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::feedback::cmd_feedback_list(ws.as_deref(), label, since, json)
}
