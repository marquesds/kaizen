// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core::config::Config;
use crate::proxy::opts::ProxyRunOptions;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared process state: workspace DB, loaded config, HTTP client, size limits.
pub struct ProxyState {
    pub options: Arc<ProxyRunOptions>,
    pub store_path: PathBuf,
    pub workspace: PathBuf,
    pub config: Arc<Config>,
    pub client: reqwest::Client,
}
