// SPDX-License-Identifier: AGPL-3.0-or-later
//! Two-pane TUI: session list (left) + events (right).

mod app;
mod background;
mod format;
mod input;
mod refresh;
mod render;
mod runtime;
mod view;
mod watch;
mod worker;

pub use runtime::run;

#[cfg(test)]
mod tests;
