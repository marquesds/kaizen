// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen observe` process wrapper: daemon proxy/session env.

use crate::ipc::{ObservedSession, ProxyEndpoint};
use crate::shell::cli::workspace_path;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

pub fn cmd_observe(workspace: Option<&Path>, agent: &str, argv: &[String]) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let observed = begin_observed_session(&ws, agent);
    let status = observed_command(argv)?
        .current_dir(&ws)
        .envs(observed_env(agent, &observed))
        .status()
        .with_context(|| format!("run observed command: {}", argv[0]))?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn observed_command(argv: &[String]) -> Result<Command> {
    let (program, args) = argv.split_first().context("missing observed command")?;
    let mut cmd = Command::new(program);
    cmd.args(args);
    Ok(cmd)
}

fn begin_observed_session(workspace: &Path, agent: &str) -> ObservedSession {
    let workspace = workspace.to_string_lossy().to_string();
    match crate::daemon::begin_observed_session_blocking(workspace, agent.into()) {
        Ok(session) => session,
        Err(err) => {
            eprintln!(
                "kaizen observe: daemon capture unavailable ({err:#}); running without proxy"
            );
            ObservedSession {
                session: format!("observe-{}", Uuid::now_v7()),
                proxies: Vec::new(),
            }
        }
    }
}

fn observed_env(agent: &str, observed: &ObservedSession) -> Vec<(String, String)> {
    let mut env = session_env(&observed.session);
    for endpoint in &observed.proxies {
        env.extend(observed_session_env(agent, endpoint, &observed.session));
    }
    dedupe_env(env)
}

fn session_env(session: &str) -> Vec<(String, String)> {
    vec![
        ("KAIZEN_SESSION_KEY".into(), session.into()),
        ("X_KAIZEN_SESSION".into(), session.into()),
    ]
}

/// Env vars for one daemon proxy endpoint.
pub fn observed_session_env(
    agent: &str,
    endpoint: &ProxyEndpoint,
    session: &str,
) -> Vec<(String, String)> {
    let mut env = session_env(session);
    if endpoint_applies(agent, endpoint, "openai")
        && let Some(base) = &endpoint.v1_base_url
    {
        env.push(("OPENAI_BASE_URL".into(), base.clone()));
    }
    if endpoint_applies(agent, endpoint, "anthropic") {
        env.push(("ANTHROPIC_BASE_URL".into(), endpoint.base_url.clone()));
    }
    env
}

fn endpoint_applies(agent: &str, endpoint: &ProxyEndpoint, provider: &str) -> bool {
    if endpoint.provider != provider {
        return false;
    }
    match agent.to_ascii_lowercase().as_str() {
        "codex" => provider == "openai",
        "claude" => provider == "anthropic",
        _ => true,
    }
}

fn dedupe_env(env: Vec<(String, String)>) -> Vec<(String, String)> {
    env.into_iter()
        .fold(
            std::collections::BTreeMap::new(),
            |mut acc, (key, value)| {
                acc.insert(key, value);
                acc
            },
        )
        .into_iter()
        .collect()
}
