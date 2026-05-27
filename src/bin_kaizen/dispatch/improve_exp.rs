use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;
use std::path::PathBuf;

pub(super) fn exp(cmd: ExpCommand) -> anyhow::Result<()> {
    use kaizen::shell::exp;
    match cmd {
        ExpCommand::New(args) => {
            let args = *args;
            let ws = resolve_ws(args.workspace.as_deref(), args.project.as_deref())?;
            exp::cmd_new(
                ws.as_deref(),
                exp::NewArgs {
                    name: args.name,
                    hypothesis: args.hypothesis,
                    change: args.change,
                    metric: args.metric,
                    bind: args.bind,
                    duration_days: args.duration_days,
                    target_pct: args.target_pct,
                    control_commit: args.control_commit,
                    treatment_commit: args.treatment_commit,
                    control_branch: args.control_branch,
                    treatment_branch: args.treatment_branch,
                    control_fingerprint: args.control_fingerprint,
                    treatment_fingerprint: args.treatment_fingerprint,
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
