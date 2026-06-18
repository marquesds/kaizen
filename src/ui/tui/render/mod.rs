// SPDX-License-Identifier: AGPL-3.0-or-later

mod events;
mod metrics;
mod sessions;
mod status;

use super::app::App;
use super::view::render_dashboard;
use ratatui::layout::{Constraint, Direction, Layout};

pub(super) fn draw(frame: &mut ratatui::Frame, app: &App) {
    if app.show_help {
        draw_help(frame, app);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(chunks[1]);
    render_dashboard(frame, app.visualization.as_ref(), chunks[0]);
    sessions::draw_sessions(frame, app, panes[0]);
    events::draw_events(frame, app, panes[1]);
    status::draw_statusbar(frame, app, chunks[2]);
}

fn draw_help(frame: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());
    status::draw_help(frame, chunks[0]);
    status::draw_statusbar(frame, app, chunks[1]);
}
