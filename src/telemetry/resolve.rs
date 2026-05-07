// SPDX-License-Identifier: AGPL-3.0-or-later
//! Env + TOML resolution for third-party keys (see `TelemetryConfig`).

use crate::core::config::ExporterConfig;

const KAIZEN: &str = "KAIZEN_";

/// Resolve env, preferring standard names, then `KAIZEN_` + same suffix.
pub(crate) fn env_two(std_key: &str, kaizen_key: &str) -> Option<String> {
    std::env::var(std_key)
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var(kaizen_key).ok().filter(|s| !s.is_empty()))
}

/// Effective PostHog settings after env overlay.
pub struct PostHogResolved {
    pub host: String,
    pub project_api_key: String,
}

/// Effective Datadog settings after env overlay. `app_key` is required for query/pull (Logs
/// Search API v2) but not for log intake; we keep it optional and surface a clear error in
/// `pull` when missing.
pub struct DatadogResolved {
    pub site: String,
    pub api_key: String,
    pub app_key: Option<String>,
}

/// Effective OTLP push endpoint.
pub struct OtlpResolved {
    pub endpoint: String,
}

impl PostHogResolved {
    pub fn from_config(c: &ExporterConfig) -> Option<Self> {
        let (host, key_opt) = match c {
            ExporterConfig::PostHog {
                host,
                project_api_key,
                ..
            } => (host.as_deref(), project_api_key.as_deref()),
            _ => return None,
        };
        let project_api_key = key_opt
            .map(String::from)
            .or_else(|| env_two("POSTHOG_API_KEY", "KAIZEN_POSTHOG_API_KEY"))?;
        let host = host
            .map(String::from)
            .or_else(|| env_two("POSTHOG_HOST", "KAIZEN_POSTHOG_HOST"))
            .unwrap_or_else(|| "https://us.i.posthog.com".to_string());
        Some(Self {
            host,
            project_api_key,
        })
    }

    /// Query / pull: API key and host from env only (no TOML row required).
    pub fn from_env_only() -> Option<Self> {
        let project_api_key = env_two("POSTHOG_API_KEY", "KAIZEN_POSTHOG_API_KEY")?;
        let host = env_two("POSTHOG_HOST", "KAIZEN_POSTHOG_HOST")
            .unwrap_or_else(|| "https://us.i.posthog.com".to_string());
        Some(Self {
            host,
            project_api_key,
        })
    }
}

impl DatadogResolved {
    pub fn from_config(c: &ExporterConfig) -> Option<Self> {
        let (site, key_opt) = match c {
            ExporterConfig::Datadog { site, api_key, .. } => (site.as_deref(), api_key.as_deref()),
            _ => return None,
        };
        let api_key = key_opt
            .map(String::from)
            .or_else(|| env_two("DD_API_KEY", "KAIZEN_DD_API_KEY"))?;
        let site = site
            .map(String::from)
            .or_else(|| env_two("DD_SITE", "KAIZEN_DD_SITE"))
            .unwrap_or_else(|| "datadoghq.com".to_string());
        let app_key = env_two("DD_APP_KEY", "KAIZEN_DD_APP_KEY");
        Some(Self {
            site,
            api_key,
            app_key,
        })
    }

    /// Query / pull: API key and site from env only.
    pub fn from_env_only() -> Option<Self> {
        let api_key = env_two("DD_API_KEY", "KAIZEN_DD_API_KEY")?;
        let site =
            env_two("DD_SITE", "KAIZEN_DD_SITE").unwrap_or_else(|| "datadoghq.com".to_string());
        let app_key = env_two("DD_APP_KEY", "KAIZEN_DD_APP_KEY");
        Some(Self {
            site,
            api_key,
            app_key,
        })
    }
}

impl OtlpResolved {
    pub fn from_config(c: &ExporterConfig) -> Option<Self> {
        let ep = match c {
            ExporterConfig::Otlp { endpoint, .. } => endpoint.as_deref(),
            _ => return None,
        };
        let endpoint = ep.map(String::from).or_else(|| {
            env_two(
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                "KAIZEN_OTEL_EXPORTER_OTLP_ENDPOINT",
            )
        })?;
        Some(Self { endpoint })
    }
}

/// Prevent unused `KAIZEN` const noise if we add more keys later; keeps resolution discoverable in docs.
#[allow(dead_code)]
fn _kaizen_prefix() -> &'static str {
    KAIZEN
}
