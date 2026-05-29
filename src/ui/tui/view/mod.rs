// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sparse TUI windows. Pure state; IO stays in worker.

mod dashboard;
mod detail;
mod event;
mod session;

pub use dashboard::render_dashboard;
pub use detail::{DetailData, DetailState};
pub use event::EventView;
pub use session::SessionView;

const PREFETCH: usize = 8;
const PAGE_FLOOR: usize = 24;
