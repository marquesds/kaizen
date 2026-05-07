// SPDX-License-Identifier: AGPL-3.0-or-later
//! Datadog Logs Search API v2 provider for query-back / pull.
//!
//! Endpoint: `POST {api_base}/api/v2/logs/events/search`. Both `DD-API-KEY` and
//! `DD-APPLICATION-KEY` are required for the query API; the intake only needs `DD-API-KEY`.

use crate::provider::{PullPage, PullWindow, TelemetryQueryProvider, http_timeout};
use crate::telemetry::DatadogResolved;
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use serde_json::{Value, json};

const PULL_PAGE_LIMIT: u32 = 1000;

pub struct DatadogQueryClient {
    client: Client,
    api_base: String,
    api_key: String,
    app_key: Option<String>,
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
            app_key: r.app_key.clone(),
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

    fn pull(&self, window: PullWindow, cursor: Option<&str>) -> Result<PullPage> {
        let app_key = self.app_key.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "Datadog pull requires DD_APP_KEY (Application Key) in addition to DD_API_KEY"
            )
        })?;
        let body = build_pull_body(window, cursor);
        let url = format!("{}/api/v2/logs/events/search", self.api_base);
        let mut api = HeaderValue::from_str(&self.api_key)
            .map_err(|e| anyhow::anyhow!("invalid API key: {e}"))?;
        api.set_sensitive(true);
        let mut app =
            HeaderValue::from_str(app_key).map_err(|e| anyhow::anyhow!("invalid APP key: {e}"))?;
        app.set_sensitive(true);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("DD-API-KEY", api)
            .header("DD-APPLICATION-KEY", app)
            .json(&body)
            .send()
            .context("send")?
            .error_for_status()
            .context("status")?;
        let v: Value = resp.json().context("decode")?;
        Ok(parse_pull_page(&v))
    }
}

/// Build a Logs Search v2 request: filter by `service:kaizen` over the last `window.days`.
pub(crate) fn build_pull_body(window: PullWindow, cursor: Option<&str>) -> Value {
    let from = format!("now-{}d", window.days.max(1));
    let mut body = json!({
        "filter": {
            "query": "service:kaizen",
            "from": from,
            "to": "now",
        },
        "page": { "limit": PULL_PAGE_LIMIT },
        "sort": "timestamp",
    });
    if let Some(c) = cursor
        && !c.is_empty()
    {
        body["page"]["cursor"] = Value::String(c.to_string());
    }
    body
}

/// Map DD response `{ data: [{ attributes: {...} }, ...], meta: { page: { after } } }` to
/// our cursorable `PullPage`. Items are the per-log `attributes` objects (host, message,
/// timestamp, kaizen.*).
pub(crate) fn parse_pull_page(v: &Value) -> PullPage {
    let items = v
        .get("data")
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .map(|row| row.get("attributes").cloned().unwrap_or(row.clone()))
                .collect()
        })
        .unwrap_or_default();
    let next_cursor = v
        .get("meta")
        .and_then(|m| m.get("page"))
        .and_then(|p| p.get("after"))
        .and_then(|c| c.as_str())
        .map(String::from);
    PullPage { next_cursor, items }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_pull_body_default_days_one_min() {
        let body = build_pull_body(PullWindow { days: 0 }, None);
        assert_eq!(body["filter"]["query"], "service:kaizen");
        assert_eq!(body["filter"]["from"], "now-1d");
        assert_eq!(body["filter"]["to"], "now");
        assert_eq!(body["page"]["limit"], PULL_PAGE_LIMIT);
        assert!(body["page"].get("cursor").is_none());
    }

    #[test]
    fn build_pull_body_threads_cursor() {
        let body = build_pull_body(PullWindow { days: 7 }, Some("abc"));
        assert_eq!(body["filter"]["from"], "now-7d");
        assert_eq!(body["page"]["cursor"], "abc");
    }

    #[test]
    fn parse_pull_page_extracts_attributes_and_cursor() {
        let v = json!({
            "data": [
                { "id": "1", "attributes": { "message": "hi", "host": "h1" } },
                { "id": "2", "attributes": { "message": "ho", "host": "h2" } }
            ],
            "meta": { "page": { "after": "next-cursor" } }
        });
        let page = parse_pull_page(&v);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0]["message"], "hi");
        assert_eq!(page.next_cursor.as_deref(), Some("next-cursor"));
    }

    #[test]
    fn parse_pull_page_handles_empty() {
        let v = json!({ "data": [], "meta": { "page": {} } });
        let page = parse_pull_page(&v);
        assert!(page.items.is_empty());
        assert!(page.next_cursor.is_none());
    }
}
