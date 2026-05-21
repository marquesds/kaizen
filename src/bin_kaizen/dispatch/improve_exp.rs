use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;
use std::path::PathBuf;

pub(super) fn exp(cmd: ExpCommand) -> anyhow::Result<()> {
    use kaizen::shell::exp;
    match cmd {
        ExpCommand::New {
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
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            exp::cmd_new(
                ws.as_deref(),
                exp::NewArgs {
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
                },
            )
        }
        ExpCommand::Start {
            id,
            workspace,
            project,
        } => exp_with_ws(workspace, project, |ws| exp::cmd_start(ws, &id)),
        ExpCommand::List { workspace, project } => exp_with_ws(workspace, project, exp::cmd_list),
        ExpCommand::Status {
            id,
            workspace,
            project,
        } => exp_with_ws(workspace, project, |ws| exp::cmd_status(ws, &id)),
        ExpCommand::Tag {
            id,
            session,
            variant,
            workspace,
            project,
        } => exp_with_ws(workspace, project, |ws| {
            exp::cmd_tag(ws, &id, &session, &variant)
        }),
        ExpCommand::Report {
            id,
            json,
            refresh,
            workspace,
            project,
        } => exp_with_ws(workspace, project, |ws| {
            exp::cmd_report(ws, &id, json, refresh)
        }),
        ExpCommand::Conclude {
            id,
            workspace,
            project,
        } => exp_with_ws(workspace, project, |ws| exp::cmd_conclude(ws, &id)),
        ExpCommand::Archive {
            id,
            workspace,
            project,
        } => exp_with_ws(workspace, project, |ws| exp::cmd_archive(ws, &id)),
        ExpCommand::Power {
            metric,
            baseline_n,
            refresh,
            workspace,
            project,
        } => exp_with_ws(workspace, project, |ws| {
            exp::cmd_power(ws, &metric, baseline_n, refresh)
        }),
    }
}

fn exp_with_ws<F>(workspace: Option<PathBuf>, project: Option<String>, f: F) -> anyhow::Result<()>
where
    F: FnOnce(Option<&std::path::Path>) -> anyhow::Result<()>,
{
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    f(ws.as_deref())
}
