// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::provider::{PullPage, PullWindow, TelemetryQueryProvider, http_timeout};
use crate::telemetry::DatadogResolved;
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use std::sync::Arc;

/// Datadog logs search (v1: empty `pull`; wire Logs API in a follow-up).
pub struct DatadogQueryClient {
    client: Client,
    api_base: String,
    api_key: String,
}

impl DatadogQueryClient {
    pub fn new(r: &DatadogResolved) -> Self {
        let site = r.site.trim();
        let api_base = format!("https://api.{site}");
        let client = Client::builder()
            .timeout(http_timeout())
            .build()
            .expect("reqwest for Datadog query");
        Self {
            client,
            api_base,
            api_key: r.api_key.clone(),
        }
    }
}

impl TelemetryQueryProvider for DatadogQueryClient {
    fn health(&self) -> Result<()> {
        let url = format!("{}/api/v1/validate", self.api_base);
        let mut key = HeaderValue::from_str(&self.api_key)
            .map_err(|e| anyhow::anyhow!("invalid API key: {e}"))?;
        key.set_sensitive(true);
        self.client
            .get(url)
            .header("DD-API-KEY", key)
            .send()?
            .error_for_status()?;
        Ok(())
    }

    fn schema_version(&self) -> &str {
        "datadog-v1"
    }

    fn pull(&self, _window: PullWindow, _cursor: Option<&str>) -> Result<PullPage> {
        Ok(PullPage::default())
    }
}

pub fn datadog_from_env() -> Option<Arc<dyn TelemetryQueryProvider>> {
    let r = DatadogResolved::from_env_only()?;
    Some(Arc::new(DatadogQueryClient::new(&r)) as Arc<dyn TelemetryQueryProvider>)
}
