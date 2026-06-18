// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::app::App;
use super::super::format::{event_detail_text, event_row_text, now_ms, span_depth_lines, truncate};
use super::metrics::draw_metrics;
use crate::ui::theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

pub(super) fn draw_events(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    if app.show_metrics {
        draw_metrics(frame, app, area);
        return;
    }
    let title = event_title(app);
    let border_color = border_color(app);
    let now = now_ms();
    if app.detail && app.selected_event().is_some() {
        draw_detail(frame, app, area, title, border_color, now);
        return;
    }
    if !app.detail_state.spans().is_empty() {
        draw_spans(frame, app, area, title, border_color, now);
        return;
    }
    if draw_pending_state(frame, app, area, &title, border_color) {
        return;
    }
    render_event_list(frame, app, area, title, border_color, now);
}

fn event_title(app: &App) -> String {
    let id = app.selected_id().unwrap_or("-");
    let model = app
        .selected_session()
        .and_then(|session| session.model.as_deref().filter(|model| !model.is_empty()))
        .map(|model| truncate(model, 24).to_string())
        .unwrap_or_else(|| "—".to_string());
    format!("Events — {:.18} — {}", id, model)
}

fn border_color(app: &App) -> Color {
    if app.left_focus {
        theme::BORDER_INACTIVE
    } else {
        theme::BORDER_ACTIVE
    }
}

fn draw_detail(
    frame: &mut ratatui::Frame,
    app: &App,
    area: Rect,
    title: String,
    border_color: Color,
    now: u64,
) {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(10)])
        .split(area);
    render_event_list(frame, app, split[0], title, border_color, now);
    let event = app.selected_event().expect("checked by caller");
    let block = Block::default()
        .title("Detail")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(
        Paragraph::new(event_detail_text(event, app.detail_state.leads()))
            .block(block)
            .wrap(Wrap { trim: true }),
        split[1],
    );
}

fn draw_spans(
    frame: &mut ratatui::Frame,
    app: &App,
    area: Rect,
    title: String,
    border_color: Color,
    now: u64,
) {
    let spans = app.detail_state.spans();
    let max_depth = spans.iter().map(|node| node.span.depth).max().unwrap_or(0);
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(2),
            Constraint::Length((max_depth + 3).min(8) as u16),
        ])
        .split(area);
    render_event_list(frame, app, split[0], title, border_color, now);
    let block = Block::default()
        .title("Span tree")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_INACTIVE));
    frame.render_widget(
        Paragraph::new(span_depth_lines(spans))
            .block(block)
            .wrap(Wrap { trim: false }),
        split[1],
    );
}

fn draw_pending_state(
    frame: &mut ratatui::Frame,
    app: &App,
    area: Rect,
    title: &str,
    border_color: Color,
) -> bool {
    let text = if matches!(
        app.detail_state,
        super::super::view::DetailState::Loading { .. }
    ) && app.events.total_loaded == 0
    {
        Some("loading...".to_string())
    } else if app.events.total_loaded == 0 {
        app.detail_state.error_message().map(ToString::to_string)
    } else {
        None
    };
    text.map(|text| draw_message(frame, area, title, border_color, text))
        .is_some()
}

fn draw_message(
    frame: &mut ratatui::Frame,
    area: Rect,
    title: &str,
    border_color: Color,
    text: String,
) {
    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_event_list(
    frame: &mut ratatui::Frame,
    app: &App,
    area: Rect,
    title: String,
    border_color: Color,
    now: u64,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let height = area.height.saturating_sub(2) as usize;
    let items = event_items(app, height, now);
    let mut state = ListState::default();
    state.select(app.events.selected_local_index(height));
    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
        area,
        &mut state,
    );
}

fn event_items(app: &App, height: usize, now: u64) -> Vec<ListItem<'static>> {
    app.events
        .visible_rows(height)
        .into_iter()
        .map(|(index, row)| {
            row.map(|event| ListItem::new(event_row_text(now, event, app.detail_state.leads())))
                .unwrap_or_else(|| ListItem::new(format!("{index:>6}  loading...")))
        })
        .collect()
}
