// SPDX-License-Identifier: AGPL-3.0-or-later
//! Local Anthropic API-compatible HTTP forwarder + `EventSource::Proxy` in SQLite. See `docs/llm-proxy.md`.

mod forward;
mod http_ext;
mod opts;
mod record;
mod server;
mod sse;
mod state;
mod transform;

pub use opts::ProxyRunOptions;
pub use server::run;
pub use state::ProxyState;
