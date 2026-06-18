// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::app::App;
use super::super::format::{model_suffix, now_ms, session_status_letter, time_ago_label};
use crate::core::event::SessionRecord;
use crate::ui::theme;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use std::collections::HashMap;

pub(super) fn draw_sessions(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(session_title(app))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color(app)));
    let height = area.height.saturating_sub(2) as usize;
    let items = session_items(app, height);
    let mut state = ListState::default();
    state.select(app.sessions.selected_local_index(height));
    frame.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
        area,
        &mut state,
    );
}

fn border_color(app: &App) -> Color {
    if app.left_focus {
        theme::BORDER_ACTIVE
    } else {
        theme::BORDER_INACTIVE
    }
}

fn session_title(app: &App) -> String {
    let count = session_count(app);
    if app.agent_filter.is_empty() {
        format!("Sessions ({count})")
    } else {
        format!("Sessions {count} (agent prefix: {:?})", app.agent_filter)
    }
}

fn session_count(app: &App) -> String {
    let loaded = app.sessions.loaded_count();
    if loaded < app.sessions.total {
        return format!("{loaded}/{} loaded", app.sessions.total);
    }
    app.sessions.total.to_string()
}

fn session_items<'a>(app: &'a App, height: usize) -> Vec<ListItem<'a>> {
    if app.sessions.loaded_once() && app.sessions.total == 0 {
        return vec![ListItem::new("No sessions found")];
    }
    let now = now_ms();
    app.sessions
        .visible_rows(height)
        .into_iter()
        .map(|(index, row)| {
            row.map(|session| session_item(now, session, &app.feedback_scores))
                .unwrap_or_else(|| ListItem::new(format!("{index:>6}  loading...")))
        })
        .collect()
}

fn session_item<'a>(
    now: u64,
    session: &'a SessionRecord,
    feedback_scores: &HashMap<String, u8>,
) -> ListItem<'a> {
    let status = format!("{:?}", session.status);
    let age = time_ago_label(now, session.started_at_ms);
    let model = model_suffix(&session.model);
    let mut spans = session_spans(session, &status, &age, &model);
    if let Some(score) = feedback_score(session, feedback_scores) {
        spans.push(Span::raw(" "));
        spans.push(score);
    }
    ListItem::new(Line::from(spans))
}

fn session_spans<'a>(
    session: &'a SessionRecord,
    status: &str,
    age: &str,
    model: &str,
) -> Vec<Span<'a>> {
    vec![
        Span::styled(
            format!("{:.10}", session.id),
            Style::default().fg(Color::Gray),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{:.7}", session.agent),
            Style::default().fg(theme::agent_color(&session.agent)),
        ),
        Span::raw(" "),
        Span::styled(
            session_status_letter(session).to_string(),
            Style::default().fg(theme::status_color(status)),
        ),
        Span::raw(" "),
        Span::styled(age.to_string(), Style::default().fg(Color::White)),
        Span::styled(model.to_string(), Style::default().fg(Color::Gray)),
    ]
}

fn feedback_score<'a>(session: &SessionRecord, scores: &HashMap<String, u8>) -> Option<Span<'a>> {
    scores.get(&session.id).map(|score| {
        let color = match score {
            1..=2 => Color::Red,
            3 => Color::Yellow,
            _ => Color::Green,
        };
        Span::styled(format!("★{score}"), Style::default().fg(color))
    })
}
