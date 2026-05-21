use crate::bin_kaizen::args::*;
use crate::bin_kaizen::dispatch::common::ws;
use crate::bin_kaizen::workspace::resolve_ws;
use kaizen::DataSource;
use std::path::PathBuf;

pub(super) fn cases(cmd: CasesCommand) -> anyhow::Result<()> {
    match cmd {
        CasesCommand::Mine(a) => {
            kaizen::shell::cases::cmd_cases_mine(ws(&a.ws)?.as_deref(), a.since.as_deref(), a.json)
        }
        CasesCommand::Create {
            session,
            reason,
            label,
            json,
            ws: f,
        } => kaizen::shell::cases::cmd_cases_create(
            ws(&f)?.as_deref(),
            &session,
            &reason,
            label,
            json,
        ),
        CasesCommand::List {
            status,
            json,
            ws: f,
        } => kaizen::shell::cases::cmd_cases_list(ws(&f)?.as_deref(), status, json),
        CasesCommand::Show(a) => {
            kaizen::shell::cases::cmd_cases_show(ws(&a.ws)?.as_deref(), &a.id, a.json)
        }
        CasesCommand::Archive(a) => {
            kaizen::shell::cases::cmd_cases_archive(ws(&a.ws)?.as_deref(), &a.id)
        }
    }
}

pub(super) fn rules(cmd: RulesCommand) -> anyhow::Result<()> {
    match cmd {
        RulesCommand::Create {
            name,
            filter,
            action,
            message,
            ws: f,
        } => kaizen::shell::rules::cmd_rules_create(
            ws(&f)?.as_deref(),
            &name,
            &filter,
            &action,
            message,
        ),
        RulesCommand::List(a) => {
            kaizen::shell::rules::cmd_rules_list(ws(&a.ws)?.as_deref(), a.json)
        }
        RulesCommand::Run {
            since,
            dry_run,
            json,
            ws: f,
        } => {
            kaizen::shell::rules::cmd_rules_run(ws(&f)?.as_deref(), since.as_deref(), dry_run, json)
        }
        RulesCommand::Enable(a) => {
            kaizen::shell::rules::cmd_rules_enable(ws(&a.ws)?.as_deref(), &a.id, true)
        }
        RulesCommand::Disable(a) => {
            kaizen::shell::rules::cmd_rules_enable(ws(&a.ws)?.as_deref(), &a.id, false)
        }
    }
}

pub(super) fn alerts(cmd: AlertsCommand) -> anyhow::Result<()> {
    match cmd {
        AlertsCommand::Check { days, json, ws: f } => {
            kaizen::shell::alerts::cmd_alerts_check(ws(&f)?.as_deref(), days, json)
        }
    }
}

pub(super) fn review(cmd: ReviewCommand) -> anyhow::Result<()> {
    match cmd {
        ReviewCommand::List {
            status,
            json,
            ws: f,
        } => kaizen::shell::review::cmd_review_list(ws(&f)?.as_deref(), status, json),
        ReviewCommand::Show(a) => {
            kaizen::shell::review::cmd_review_show(ws(&a.ws)?.as_deref(), &a.id, a.json)
        }
        ReviewCommand::Resolve(a) => {
            kaizen::shell::review::cmd_review_resolve(ws(&a.ws)?.as_deref(), &a.id)
        }
        ReviewCommand::Dismiss(a) => {
            kaizen::shell::review::cmd_review_dismiss(ws(&a.ws)?.as_deref(), &a.id)
        }
    }
}

pub(super) fn eval(cmd: EvalCommand) -> anyhow::Result<()> {
    match cmd {
        EvalCommand::Run {
            workspace,
            project,
            since_days,
            dry_run,
            json,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::eval::cmd_eval_run(ws.as_deref(), since_days, dry_run, json)
        }
        EvalCommand::List {
            workspace,
            project,
            min_score,
            json,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::eval::cmd_eval_list(ws.as_deref(), min_score, json)
        }
        EvalCommand::Prompt {
            workspace,
            project,
            session_id,
            rubric,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::eval::cmd_eval_prompt(ws.as_deref(), &session_id, &rubric)
        }
    }
}

pub(super) struct RetroRequest {
    pub(super) days: u32,
    pub(super) dry_run: bool,
    pub(super) json: bool,
    pub(super) force: bool,
    pub(super) workspace: Option<PathBuf>,
    pub(super) project: Option<String>,
    pub(super) refresh: bool,
    pub(super) source: DataSource,
}

pub(super) fn retro(req: RetroRequest) -> anyhow::Result<()> {
    let ws = resolve_ws(req.workspace.as_deref(), req.project.as_deref())?;
    kaizen::shell::retro::cmd_retro(
        ws.as_deref(),
        req.days,
        req.dry_run,
        req.json,
        req.force,
        req.refresh,
        req.source,
    )
}
