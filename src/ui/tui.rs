// SPDX-License-Identifier: AGPL-3.0-or-later
//! Two-pane TUI: session list (left) + events (right).

use crate::core::event::{Event, SessionRecord, SessionStatus};
use crate::metrics::types::MetricsReport;
use crate::metrics::{index, report};
use crate::store::Store;
use crate::ui::theme;
use anyhow::Result;
use crossterm::{
    event::{self as cxev, Event as CxEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::collections::HashMap;
use std::path::Path;
use time::OffsetDateTime;
use tokio::sync::broadcast;
use tokio::time::{Duration, interval};

const THIRTY_DAYS_SEC: u64 = 30 * 24 * 3600;
/// Timestamps below this are treated as Unix **seconds** when `now` looks like ms (ingest mistakes).
const MS_HEURISTIC_THRESHOLD: u64 = 1_000_000_000_000;

struct App {
    sessions: Vec<SessionRecord>,
    events: Vec<Event>,
    /// `tool_call_id` -> `lead_time_ms` for the selected session (from `tool_spans`).
    tool_lead_by_call: HashMap<String, u64>,
    sel_session: usize,
    sel_event: usize,
    left_focus: bool,
    show_help: bool,
    /// When true, show payload / fields for `events[sel_event]` in a strip under the list.
    detail: bool,
    show_metrics: bool,
    metrics: Option<MetricsReport>,
    pulse: bool,
    store: Store,
    workspace: String,
}

impl App {
    fn open(workspace: &Path) -> Result<Self> {
        let db = workspace.join(".kaizen/kaizen.db");
        let store = Store::open(&db)?;
        let ws = workspace.to_string_lossy().to_string();
        let sessions = store.list_sessions(&ws)?;
        let _ = index::ensure_indexed(&store, workspace, false);
        let metrics = report::build_report(&store, &ws, 7).ok();
        let mut app = Self {
            sessions,
            events: vec![],
            tool_lead_by_call: HashMap::new(),
            sel_session: 0,
            sel_event: 0,
            left_focus: true,
            show_help: false,
            detail: false,
            show_metrics: false,
            metrics,
            pulse: false,
            store,
            workspace: ws,
        };
        app.refresh()?;
        Ok(app)
    }

    fn refresh(&mut self) -> Result<()> {
        self.sessions = self.store.list_sessions(&self.workspace)?;
        self.pulse = !self.pulse;
        if let Some(s) = self.sessions.get(self.sel_session) {
            self.events = self.store.list_events_for_session(&s.id)?;
            self.tool_lead_by_call.clear();
            for row in self.store.tool_spans_for_session(&s.id)? {
                if let (Some(id), Some(lt)) = (row.tool_call_id, row.lead_time_ms) {
                    self.tool_lead_by_call.insert(id, lt);
                }
            }
        } else {
            self.events.clear();
            self.tool_lead_by_call.clear();
        }
        self.sel_event = self.sel_event.min(self.events.len().saturating_sub(1));
        self.metrics = report::build_report(&self.store, &self.workspace, 7).ok();
        Ok(())
    }

    fn selected_session(&self) -> Option<&SessionRecord> {
        self.sessions.get(self.sel_session)
    }

    fn selected_id(&self) -> Option<&str> {
        self.selected_session().map(|s| s.id.as_str())
    }

    fn selected_event(&self) -> Option<&Event> {
        self.events.get(self.sel_event)
    }
}

/// Relative or absolute time; guards bogus `ts_ms` and very old events.
fn time_ago_label(now_ms: u64, ts_ms: u64) -> String {
    if ts_ms == 0 {
        return "?".to_string();
    }
    let mut ts = ts_ms;
    if ts < MS_HEURISTIC_THRESHOLD && now_ms >= MS_HEURISTIC_THRESHOLD {
        ts = ts.saturating_mul(1000);
    }
    let diff_sec = now_ms.saturating_sub(ts) / 1000;
    if diff_sec > THIRTY_DAYS_SEC {
        return abs_ts_label(ts);
    }
    match diff_sec {
        0 => "just now".to_string(),
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}

fn abs_ts_label(ts_ms: u64) -> String {
    let Ok(dt) = OffsetDateTime::from_unix_timestamp((ts_ms / 1000) as i64) else {
        return "?".to_string();
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute()
    )
}

fn truncate(s: &str, max: usize) -> &str {
    if s.chars().count() <= max {
        return s;
    }
    s.char_indices()
        .nth(max.saturating_sub(1))
        .map(|(i, _)| &s[..i])
        .unwrap_or(s)
}

fn model_suffix(model: &Option<String>) -> String {
    const MAX: usize = 20;
    match model {
        Some(m) if !m.is_empty() => format!(" {}", truncate(m, MAX)),
        _ => " —".to_string(),
    }
}

fn session_status_letter(s: &SessionRecord) -> char {
    match s.status {
        SessionStatus::Running => 'R',
        SessionStatus::Waiting => 'W',
        SessionStatus::Idle => 'I',
        SessionStatus::Done => 'D',
    }
}

fn format_event_tokens(e: &Event) -> Option<String> {
    let mut out = String::new();
    match (e.tokens_in, e.tokens_out) {
        (Some(a), Some(b)) => out = format!("{a}/{b}"),
        (Some(a), None) => out = a.to_string(),
        (None, Some(b)) => out = b.to_string(),
        (None, None) => {}
    }
    if let Some(r) = e.reasoning_tokens {
        if out.is_empty() {
            out = format!("r{r}");
        } else {
            out = format!("{out}+r{r}");
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn event_row_text(now_ms: u64, e: &Event, lead: &HashMap<String, u64>) -> String {
    let age = time_ago_label(now_ms, e.ts_ms);
    let tool = e.tool.as_deref().unwrap_or("-");
    let lead_s = e
        .tool_call_id
        .as_ref()
        .and_then(|id| lead.get(id).copied())
        .map(|ms| format!("{ms}ms"))
        .unwrap_or_else(|| "—".to_string());
    let tok = format_event_tokens(e)
        .map(|s| format!(" tok={s}"))
        .unwrap_or_default();
    format!("{age}  {kind:?}  {tool}{tok}  {lead_s}", kind = e.kind)
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    if app.show_help {
        draw_help(f);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(chunks[0]);

    draw_sessions(f, app, panes[0]);
    draw_events(f, app, panes[1]);
    draw_statusbar(f, app, chunks[1]);
}

fn draw_sessions(f: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let border_color = if app.left_focus {
        theme::BORDER_ACTIVE
    } else {
        theme::BORDER_INACTIVE
    };
    let block = Block::default()
        .title(format!("Sessions ({})", app.sessions.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let now = now_ms();
    let items: Vec<ListItem> = app
        .sessions
        .iter()
        .map(|s| {
            let st = format!("{:?}", s.status);
            let st_color = theme::status_color(&st);
            let age = time_ago_label(now, s.started_at_ms);
            let tag = session_status_letter(s);
            let m = model_suffix(&s.model);
            let line = Line::from(vec![
                Span::styled(format!("{:.10}", s.id), Style::default().fg(Color::Gray)),
                Span::raw(" "),
                Span::styled(
                    format!("{:.7}", s.agent),
                    Style::default().fg(theme::agent_color(&s.agent)),
                ),
                Span::raw(" "),
                Span::styled(format!("{tag}"), Style::default().fg(st_color)),
                Span::raw(" "),
                Span::styled(age, Style::default().fg(Color::White)),
                Span::styled(m, Style::default().fg(Color::Gray)),
            ]);
            ListItem::new(line)
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.sel_session));
    f.render_stateful_widget(
        ratatui::widgets::List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
        area,
        &mut state,
    );
}

fn draw_events(f: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    if app.show_metrics {
        draw_metrics(f, app, area);
        return;
    }
    let id = app.selected_id().unwrap_or("-");
    let model = app
        .selected_session()
        .and_then(|s| s.model.as_deref().filter(|m| !m.is_empty()))
        .map(|m| truncate(m, 24).to_string())
        .unwrap_or_else(|| "—".to_string());
    let border_color = if !app.left_focus {
        theme::BORDER_ACTIVE
    } else {
        theme::BORDER_INACTIVE
    };
    let title = format!("Events — {:.18} — {}", id, model);
    let now = now_ms();
    if app.detail {
        if let (true, Some(ev)) = (!app.events.is_empty(), app.selected_event()) {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(2), Constraint::Length(10)])
                .split(area);
            let items: Vec<ListItem> = app
                .events
                .iter()
                .map(|e| {
                    let row = event_row_text(now, e, &app.tool_lead_by_call);
                    ListItem::new(row)
                })
                .collect();
            let mut state = ListState::default();
            state.select(Some(app.sel_event));
            let list_block = Block::default()
                .title(title.clone())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));
            f.render_stateful_widget(
                List::new(items)
                    .block(list_block)
                    .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
                split[0],
                &mut state,
            );
            let detail = event_detail_text(ev, &app.tool_lead_by_call);
            let det_block = Block::default()
                .title("Detail")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));
            f.render_widget(
                Paragraph::new(detail)
                    .block(det_block)
                    .wrap(Wrap { trim: true }),
                split[1],
            );
            return;
        }
    }
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let items: Vec<ListItem> = app
        .events
        .iter()
        .map(|e| {
            let row = event_row_text(now, e, &app.tool_lead_by_call);
            ListItem::new(row)
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.sel_event));
    f.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
        area,
        &mut state,
    );
}

fn event_detail_text(ev: &Event, lead: &HashMap<String, u64>) -> String {
    let lead_s = ev
        .tool_call_id
        .as_ref()
        .and_then(|id| lead.get(id).copied())
        .map(|ms| format!("{ms}ms"))
        .unwrap_or_else(|| "—".to_string());
    let head = format!(
        "seq={}  kind={:?}  tool={}  call_id={}  in={:?} out={:?} r={:?}  cost_e6={:?}  lead={}\n",
        ev.seq,
        ev.kind,
        ev.tool.as_deref().unwrap_or("-"),
        ev.tool_call_id.as_deref().unwrap_or("—"),
        ev.tokens_in,
        ev.tokens_out,
        ev.reasoning_tokens,
        ev.cost_usd_e6,
        lead_s
    );
    let json = serde_json::to_string_pretty(&ev.payload).unwrap_or_else(|_| ev.payload.to_string());
    head + &json
}

fn draw_metrics(f: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let block = Block::default()
        .title("Metrics")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER_ACTIVE));
    let mut lines = vec!["Slow tools".to_string()];
    if let Some(metrics) = &app.metrics {
        for row in metrics.slowest_tools.iter().take(4) {
            let p95 = row
                .p95_ms
                .map(|v| format!("{v}ms"))
                .unwrap_or_else(|| "-".into());
            lines.push(format!("{} p95={} tok={}", row.tool, p95, row.total_tokens));
        }
        lines.push(String::new());
        lines.push("Hot files".into());
        for row in metrics.hottest_files.iter().take(4) {
            lines.push(format!("{} {}", row.value, row.path));
        }
    }
    f.render_widget(Paragraph::new(lines.join("\n")).block(block), area);
}

fn draw_statusbar(f: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let pulse = if app.pulse { "●" } else { "○" };
    let text = format!(
        "LIVE {pulse}  j/k move  |  Tab pane  |  m metrics  |  Enter detail  |  ? help  |  q quit"
    );
    f.render_widget(Paragraph::new(text), area);
}

fn draw_help(f: &mut ratatui::Frame) {
    let text = "j/k ↑/↓  move in focused pane  |  g/G first/last  |  Tab  switch pane\n\
                Enter  toggle event detail  |  Esc  back  |  r  refresh  |  q  quit";
    let block = Block::default().title("Help").borders(Borders::ALL);
    f.render_widget(Paragraph::new(text).block(block), f.area());
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Entry point. Opens terminal, polls SQLite every 500 ms, handles keys.
pub async fn run(workspace: &Path) -> Result<()> {
    let mut app = App::open(workspace)?;
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    std::panic::set_hook(Box::new(|_| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }));

    let (tx, _rx) = broadcast::channel::<()>(1);
    let tx2 = tx.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(500));
        loop {
            ticker.tick().await;
            let _ = tx2.send(());
        }
    });
    let mut rx = tx.subscribe();

    loop {
        terminal.draw(|f| draw(f, &app))?;
        tokio::select! {
            _ = rx.recv() => { let _ = app.refresh(); }
            _ = tokio::task::spawn_blocking(|| { cxev::poll(Duration::from_millis(50)) }) => {
                if cxev::poll(Duration::ZERO)?
                    && let CxEvent::Key(k) = cxev::read()?
                {
                    if k.kind != KeyEventKind::Press { continue; }
                    match k.code {
                        KeyCode::Char('q') | KeyCode::Esc if !app.detail && !app.show_help => break,
                        KeyCode::Char('q') if app.show_help => { app.show_help = false; }
                        KeyCode::Char('q') => { app.detail = false; app.show_help = false; }
                        KeyCode::Esc | KeyCode::Backspace => {
                            app.detail = false;
                            app.show_help = false;
                        }
                        KeyCode::Char('?') => app.show_help = !app.show_help,
                        KeyCode::Char('m') => app.show_metrics = !app.show_metrics,
                        KeyCode::Tab => {
                            app.left_focus = !app.left_focus;
                        }
                        KeyCode::Char('r') => { let _ = app.refresh(); }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if app.show_metrics || app.left_focus {
                                if app.sel_session + 1 < app.sessions.len() {
                                    app.sel_session += 1;
                                }
                            } else if app.sel_event + 1 < app.events.len() {
                                app.sel_event += 1;
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if app.show_metrics || app.left_focus {
                                if app.sel_session > 0 {
                                    app.sel_session -= 1;
                                }
                            } else if app.sel_event > 0 {
                                app.sel_event -= 1;
                            }
                        }
                        KeyCode::Char('g') => {
                            if app.show_metrics || app.left_focus {
                                app.sel_session = 0;
                            } else {
                                app.sel_event = 0;
                            }
                        }
                        KeyCode::Char('G') => {
                            if app.show_metrics || app.left_focus {
                                app.sel_session = app.sessions.len().saturating_sub(1);
                            } else {
                                app.sel_event = app.events.len().saturating_sub(1);
                            }
                        }
                        KeyCode::Enter if !app.events.is_empty() && !app.show_metrics => {
                            app.detail = !app.detail;
                        }
                        _ => {}
                    }
                    // Reload events when the selected session index changes.
                    if matches!(k.code,
                        KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Up | KeyCode::Down
                        | KeyCode::Char('g') | KeyCode::Char('G')
                    ) && (app.show_metrics || app.left_focus)
                    {
                        let _ = app.refresh();
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_ago_just_now() {
        assert_eq!(time_ago_label(10_000, 10_000), "just now");
    }

    #[test]
    fn time_ago_treats_small_ts_as_seconds() {
        let now = 1_700_000_000_000u64;
        let ts_sec = 1_700_000_000u64;
        let label = time_ago_label(now, ts_sec);
        assert!(
            !label.contains('?'),
            "expected relative label, got {label:?}"
        );
    }

    #[test]
    fn time_ago_old_uses_absolute() {
        let now = 1_700_000_000_000u64;
        let old = now - (40u64 * 24 * 3600 * 1000);
        let label = time_ago_label(now, old);
        assert!(label.contains('-'), "expected date-like label: {label}");
    }
}
