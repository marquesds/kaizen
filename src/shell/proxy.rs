// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen proxy run` — see `docs/llm-proxy.md`.

use crate::core::config;
use crate::proxy::ProxyRunOptions;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// Run the local LLM forwarder until `Ctrl+C` or error.
pub fn cmd_proxy_run(
    workspace: Option<&Path>,
    listen: Option<String>,
    upstream: Option<String>,
) -> Result<()> {
    let dir = workspace
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().expect("cwd"));
    let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
    let cfg = config::load(&dir)?;
    let o = Arc::new(ProxyRunOptions::from_config_with_overrides(
        &cfg, listen, upstream,
    ));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move { crate::proxy::run(o, dir, cfg).await })
}
