// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::provider::{PullPage, PullWindow, TelemetryQueryProvider, http_timeout};
use crate::telemetry::PostHogResolved;
use anyhow::Result;
use reqwest::blocking::Client;
use std::sync::Arc;

/// Live PostHog query client (HogQL / export to be fleshed out; v1 returns empty `pull`).
pub struct PostHogQueryClient {
    client: Client,
    base: String,
    api_key: String,
}

impl PostHogQueryClient {
    pub fn new(r: &PostHogResolved) -> Self {
        let base = r.host.trim_end_matches('/').to_string();
        let client = Client::builder()
            .timeout(http_timeout())
            .build()
            .expect("reqwest for PostHog query");
        Self {
            client,
            base,
            api_key: r.project_api_key.clone(),
        }
    }
}

impl TelemetryQueryProvider for PostHogQueryClient {
    fn health(&self) -> Result<()> {
        // Liveness: GET root (DNS + TLS). API key is checked on `pull` / capture when wired.
        self.client
            .get(format!("{}/", self.base))
            .send()?
            .error_for_status()?;
        Ok(())
    }

    fn schema_version(&self) -> &str {
        "posthog-v1"
    }

    fn pull(&self, _window: PullWindow, _cursor: Option<&str>) -> Result<PullPage> {
        let _ = &self.api_key; // used when events API is wired
        Ok(PullPage::default())
    }
}

/// Returns `None` if env keys for PostHog are missing.
pub fn posthog_from_env() -> Option<Arc<dyn TelemetryQueryProvider>> {
    let r = PostHogResolved::from_env_only()?;
    Some(Arc::new(PostHogQueryClient::new(&r)) as Arc<dyn TelemetryQueryProvider>)
}
