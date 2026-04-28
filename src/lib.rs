// SPDX-License-Identifier: AGPL-3.0-or-later

//! # kaizen
//!
//! Re-exports the internal crate graph for the `kaizen` binary, integration tests, and
//! `cargo check` of the full tree.
//!
//! # Documentation for users
//!
//! Prose lives in the repository on GitHub: [`docs/`](https://github.com/marquesds/kaizen/tree/main/docs)
//! (CLI, configuration, and the [telemetry journey](https://github.com/marquesds/kaizen/blob/main/docs/telemetry-journey.md)
//! explainer). The [docs.rs](https://docs.rs/kaizen-cli) page documents **this** Rust API; it does
//! not include the `docs/` markdown because that folder is excluded from the published crate
//! (see `exclude` in `Cargo.toml`).

pub mod collect;
pub mod core;
pub mod daemon;
pub mod eval;
pub mod feedback;
pub mod ipc;
pub mod prompt;

pub use crate::core::DataSource;
pub mod experiment;
pub mod mcp;
pub mod metrics;
pub mod provider;
pub mod proxy;
pub mod report;
pub mod retro;
pub mod shell;
pub mod store;
pub mod sync;
pub mod telemetry;
pub mod ui;
