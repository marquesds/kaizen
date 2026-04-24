// SPDX-License-Identifier: AGPL-3.0-or-later
//! Best-effort provider pull into local `remote_*` cache before read commands (when `DataSource` requests it).

use crate::core::config::Config;
use crate::core::data_source::DataSource;
use crate::provider::{PullWindow, from_config as provider_from_config};
use crate::store::Store;
use crate::store::remote_cache::{RemoteCacheStore, RemotePullState};
use anyhow::Result;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn query_provider_label(cfg: &Config) -> String {
    match cfg.telemetry.query.provider {
        crate::core::config::QueryAuthority::None => "none".into(),
        crate::core::config::QueryAuthority::Posthog => "posthog".into(),
        crate::core::config::QueryAuthority::Datadog => "datadog".into(),
    }
}

/// When `source` is not local, refresh remote cache if TTL expired or `force_refresh` (CLI `--refresh` with provider/mixed).
pub fn maybe_telemetry_pull(
    _workspace: &Path,
    store: &Store,
    cfg: &Config,
    source: DataSource,
    force_refresh: bool,
) -> Result<()> {
    if source == DataSource::Local {
        return Ok(());
    }
    let Some(p) = provider_from_config(&cfg.telemetry.query) else {
        tracing::debug!("telemetry: no query provider; skip pull");
        return Ok(());
    };
    let state = store.get_pull_state()?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let ttl_ms = (cfg.telemetry.query.cache_ttl_seconds as i64).saturating_mul(1000);
    let fresh = state
        .last_success_ms
        .map(|t| now_ms - t < ttl_ms)
        .unwrap_or(false);
    if !force_refresh && fresh {
        return Ok(());
    }
    let page = p.pull(PullWindow { days: 7 }, None)?;
    if !cfg.sync.team_id.trim().is_empty()
        && let Some(ctx) = crate::sync::ingest_ctx(cfg, _workspace.to_path_buf())
        && let Some(wh) = crate::sync::smart::workspace_hash_for(&ctx)
    {
        match crate::provider::import_pull_page_to_remote(store, &cfg.sync.team_id, &wh, &page) {
            Ok(n) if n > 0 => tracing::debug!(n, "remote_events: imported from provider pull"),
            _ => {}
        }
    }
    store.set_pull_state(&RemotePullState {
        query_provider: query_provider_label(cfg),
        cursor_json: String::new(),
        last_success_ms: Some(now_ms),
    })?;
    Ok(())
}
