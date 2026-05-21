use crate::bin_kaizen::args::*;
use crate::bin_kaizen::workspace::resolve_ws;
use clap::CommandFactory;
use std::io::{Read, Write};
use std::path::PathBuf;

pub(super) fn ingest(cmd: IngestCommand) -> anyhow::Result<()> {
    let IngestCommand::Hook {
        source,
        workspace,
        project,
    } = cmd;
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    ingest_hook(source, ws)
}

fn ingest_hook(source: Source, workspace: Option<PathBuf>) -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let src = ingest_source(source);
    if kaizen::daemon::enabled() {
        return daemon_ingest(src, input, workspace);
    }
    kaizen::shell::ingest::ingest_hook_text(src, &input, workspace)
}

fn ingest_source(source: Source) -> kaizen::shell::ingest::IngestSource {
    match source {
        Source::Cursor => kaizen::shell::ingest::IngestSource::Cursor,
        Source::Claude => kaizen::shell::ingest::IngestSource::Claude,
        Source::Vibe => kaizen::shell::ingest::IngestSource::Vibe,
    }
}

fn daemon_ingest(
    source: kaizen::shell::ingest::IngestSource,
    payload: String,
    workspace: Option<PathBuf>,
) -> anyhow::Result<()> {
    let response = kaizen::daemon::request_blocking(kaizen::ipc::DaemonRequest::IngestHook {
        source,
        payload,
        workspace: workspace.map(|p| {
            kaizen::core::paths::canonical(&p)
                .to_string_lossy()
                .to_string()
        }),
    })?;
    match response {
        kaizen::ipc::DaemonResponse::Ack { .. } => Ok(()),
        kaizen::ipc::DaemonResponse::Error { message, .. } => Err(anyhow::anyhow!(message)),
        _ => Err(anyhow::anyhow!("unexpected daemon ingest response")),
    }
}

pub(super) fn gc(
    workspace: Option<PathBuf>,
    project: Option<String>,
    days: Option<u32>,
    vacuum: bool,
) -> anyhow::Result<()> {
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::gc::cmd_gc(ws.as_deref(), days, vacuum)
}

pub(super) fn migrate(cmd: MigrateCommand) -> anyhow::Result<()> {
    match cmd {
        MigrateCommand::V2 {
            workspace,
            allow_skew,
        } => kaizen::shell::migrate::cmd_migrate_v2(workspace.as_deref(), allow_skew),
        MigrateCommand::V1 { workspace } => {
            kaizen::shell::migrate::cmd_migrate_v1(workspace.as_deref())
        }
    }
}

pub(super) fn completions(shell: CompletionShell) -> anyhow::Result<()> {
    let sh = match shell {
        CompletionShell::Bash => clap_complete::Shell::Bash,
        CompletionShell::Elvish => clap_complete::Shell::Elvish,
        CompletionShell::Fish => clap_complete::Shell::Fish,
        CompletionShell::Powershell => clap_complete::Shell::PowerShell,
        CompletionShell::Zsh => clap_complete::Shell::Zsh,
    };
    let mut cmd = Cli::command();
    clap_complete::generate(sh, &mut cmd, "kaizen", &mut std::io::stdout());
    let _ = std::io::stdout().flush();
    Ok(())
}

pub(super) fn sync(cmd: SyncCommand) -> anyhow::Result<()> {
    match cmd {
        SyncCommand::Run {
            workspace,
            project,
            once,
        } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::sync::cmd_sync_run(ws.as_deref(), once)
        }
        SyncCommand::Status { workspace, project } => {
            let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
            kaizen::shell::sync::cmd_sync_status(ws.as_deref())
        }
    }
}

pub(super) fn upgrade(from_source: bool) -> anyhow::Result<()> {
    kaizen::shell::upgrade::cmd_upgrade(from_source)
}

pub(super) fn mcp() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(kaizen::mcp::run_stdio_server())
}

pub(super) fn proxy(cmd: ProxyCommand) -> anyhow::Result<()> {
    let ProxyCommand::Run {
        listen,
        upstream,
        provider,
        workspace,
        project,
    } = cmd;
    let ws = resolve_ws(workspace.as_deref(), project.as_deref())?;
    kaizen::shell::proxy::cmd_proxy_run(ws.as_deref(), listen, upstream, provider)
}

pub(super) fn sampler_run(workspace: PathBuf, session: String, pid: u32) -> anyhow::Result<()> {
    kaizen::shell::sampler_cmd::cmd_sampler_run(&workspace, &session, pid)
}
