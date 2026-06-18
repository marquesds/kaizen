// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::app::App;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};

pub(super) fn draw_statusbar(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    frame.render_widget(Paragraph::new(status_text(app)), area);
}

fn status_text(app: &App) -> String {
    if !app.error_note.is_empty() {
        return format!(
            "Store error: {}  |  r refresh  |  ? help  |  q quit",
            app.error_note
        );
    }
    if app.filter_mode {
        return format!(
            "FILTER  type agent substring  |  Enter apply  |  Esc cancel  |  buffer: {}",
            app.filter_buf
        );
    }
    let pulse = if app.pulse { "●" } else { "○" };
    let note = if app.clipboard_note.is_empty() {
        String::new()
    } else {
        format!("  |  {}", app.clipboard_note)
    };
    format!(
        "LIVE {pulse}  j/k  Tab  m metrics  / filter  y copy id  Enter detail  ? help  q quit{note}"
    )
}

pub(super) fn draw_help(frame: &mut ratatui::Frame, area: Rect) {
    let text = "j/k ↑/↓  move in focused pane  |  g/G first/last  |  Tab  switch pane\n\
                Enter  event detail  |  m  metrics  |  r  refresh  |  /  filter\n\
                y  copy session id  |  Esc  back  |  ?  close help  |  q  quit";
    let block = Block::default().title("Help").borders(Borders::ALL);
    frame.render_widget(Paragraph::new(text).block(block), area);
}
