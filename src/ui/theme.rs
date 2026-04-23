// SPDX-License-Identifier: AGPL-3.0-or-later
//! Color constants for TUI.

use ratatui::style::Color;

pub const RUNNING: Color = Color::LightGreen;
pub const WAITING: Color = Color::Yellow;
pub const IDLE: Color = Color::Cyan;
pub const DONE: Color = Color::DarkGray;

pub const AGENT_CURSOR: Color = Color::Blue;
pub const AGENT_CLAUDE: Color = Color::Magenta;
pub const AGENT_CODEX: Color = Color::Red;
pub const AGENT_OTHER: Color = Color::White;

pub const BORDER_ACTIVE: Color = Color::White;
pub const BORDER_INACTIVE: Color = Color::DarkGray;

/// Color for agent name.
pub fn agent_color(agent: &str) -> Color {
    match agent {
        "cursor" => AGENT_CURSOR,
        "claude" => AGENT_CLAUDE,
        "codex" => AGENT_CODEX,
        _ => AGENT_OTHER,
    }
}

/// Color for status string.
pub fn status_color(status: &str) -> Color {
    match status {
        "Running" => RUNNING,
        "Waiting" => WAITING,
        "Idle" => IDLE,
        _ => DONE,
    }
}
