use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;

pub(super) fn prompt(cmd: PromptCommand) -> anyhow::Result<()> {
    match cmd {
        PromptCommand::List {
            workspace,
            project,
            json,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::prompt::cmd_prompt_list(ws.as_deref(), json)
        }
        PromptCommand::Show {
            fingerprint,
            workspace,
            project,
            json,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::prompt::cmd_prompt_show(&fingerprint, ws.as_deref(), json)
        }
        PromptCommand::Diff {
            fingerprint_a,
            fingerprint_b,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::prompt::cmd_prompt_diff(&fingerprint_a, &fingerprint_b, ws.as_deref())
        }
    }
}

pub(super) fn outcomes(cmd: OutcomesCommand) -> anyhow::Result<()> {
    match cmd {
        OutcomesCommand::Show {
            id,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::outcomes_cmd::cmd_outcomes_show(&id, ws.as_deref())
        }
        OutcomesCommand::Measure { workspace, session } => {
            kaizen::shell::outcomes_cmd::cmd_outcomes_measure(&workspace, &session)
        }
    }
}
