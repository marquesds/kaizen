use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;

pub(super) fn export(subcmd: ExportCommand) -> anyhow::Result<()> {
    match subcmd {
        ExportCommand::Atif {
            session,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::extensions::cmd_export_atif(ws.as_deref(), &session)
        }
    }
}

pub(super) fn import(subcmd: ImportCommand) -> anyhow::Result<()> {
    match subcmd {
        ImportCommand::Atif {
            file,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::extensions::cmd_import_atif(ws.as_deref(), &file)
        }
        ImportCommand::Jsonl {
            file,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::extensions::cmd_import_jsonl(ws.as_deref(), &file)
        }
    }
}

pub(super) fn verify(subcmd: VerifyCommand) -> anyhow::Result<()> {
    match subcmd {
        VerifyCommand::HashChain {
            session,
            workspace,
            project,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::extensions::cmd_verify_hash_chain(ws.as_deref(), session.as_deref())
        }
    }
}
