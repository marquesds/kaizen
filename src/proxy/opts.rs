// SPDX-License-Identifier: AGPL-3.0-or-later
//! Effective `kaizen proxy` options from TOML + CLI overrides.

use crate::core::config::{Config, ContextPolicy};

/// Resolved after merging workspace + user config; CLI can override `listen` / `upstream`.
pub struct ProxyRunOptions {
    pub listen: String,
    pub upstream: String,
    pub provider: String,
    pub minify_json: bool,
    pub compress_transport: bool,
    pub max_response_bytes: u64,
    pub max_request_bytes: u64,
    pub context_policy: ContextPolicy,
}

impl ProxyRunOptions {
    pub fn from_config(cfg: &Config) -> Self {
        let p = &cfg.proxy;
        Self {
            listen: p.listen.clone(),
            upstream: provider_upstream(&p.provider, &p.upstream),
            provider: p.provider.clone(),
            minify_json: p.minify_json,
            compress_transport: p.compress_transport,
            max_response_bytes: u64::from(p.max_response_body_mb) * 1024 * 1024,
            max_request_bytes: u64::from(p.max_request_body_mb) * 1024 * 1024,
            context_policy: p.context_policy.clone(),
        }
    }

    /// CLI `--listen` / `--upstream` win when set.
    pub fn from_config_with_overrides(
        cfg: &Config,
        listen: Option<String>,
        upstream: Option<String>,
        provider: Option<String>,
    ) -> Self {
        let mut o = Self::from_config(cfg);
        if let Some(s) = listen {
            o.listen = s;
        }
        if let Some(s) = provider {
            o.provider = s;
            o.upstream = provider_upstream(&o.provider, &o.upstream);
        }
        if let Some(s) = upstream {
            o.upstream = s;
        }
        o
    }
}

fn provider_upstream(provider: &str, upstream: &str) -> String {
    if provider.eq_ignore_ascii_case("openai") && upstream == "https://api.anthropic.com" {
        "https://api.openai.com".into()
    } else {
        upstream.to_string()
    }
}
