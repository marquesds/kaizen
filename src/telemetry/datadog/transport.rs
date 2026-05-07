// SPDX-License-Identifier: AGPL-3.0-or-later
//! Datadog Logs API HTTP transport. Pure builder lives in [`super::build`]; this file is the
//! imperative shell that posts each chunk and reports first error while logging the rest.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::HeaderValue;
use serde_json::Value;

/// POST one chunk per request; on partial failure log per-chunk index then return first error.
/// The DD intake responds 202 on accept (non-2xx surfaces as `error_for_status`).
pub fn post_chunks(
    client: &Client,
    url: &str,
    api_key: &str,
    chunks: Vec<Vec<Value>>,
) -> Result<()> {
    let mut first_err: Option<anyhow::Error> = None;
    let total = chunks.len();
    for (idx, chunk) in chunks.into_iter().enumerate() {
        match post_one(client, url, api_key, &chunk) {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(chunk = idx + 1, total, error = %e, "datadog chunk POST failed");
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    if let Some(e) = first_err {
        return Err(e.context("datadog logs POST"));
    }
    Ok(())
}

fn post_one(client: &Client, url: &str, api_key: &str, chunk: &[Value]) -> Result<()> {
    let mut key = HeaderValue::from_str(api_key)
        .map_err(|e| anyhow::anyhow!("invalid Datadog API key: {e}"))?;
    key.set_sensitive(true);
    client
        .post(url)
        .header("Content-Type", "application/json")
        .header("DD-API-KEY", key)
        .json(&chunk)
        .send()
        .context("send")?
        .error_for_status()
        .context("status")?;
    Ok(())
}
