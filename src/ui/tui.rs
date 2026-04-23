//! Two-pane TUI: session list (left) + events (right).

use crate::core::event::{Event, SessionRecord};
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
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::path::Path;
use tokio::sync::broadcast;
use tokio::time::{Duration, interval};

struct App {
    sessions: Vec<SessionRecord>,
    events: Vec<Event>,
    sel_session: usize,
    left_focus: bool,
    show_help: bool,
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
        Ok(Self {
            sessions,
            events: vec![],
            sel_session: 0,
            left_focus: true,
            show_help: false,
            detail: false,
            show_metrics: false,
            metrics,
            pulse: false,
            store,
            workspace: ws,
        })
    }

    fn refresh(&mut self) -> Result<()> {
        self.sessions = self.store.list_sessions(&self.workspace)?;
        self.pulse = !self.pulse;
        if let Some(s) = self.sessions.get(self.sel_session) {
            self.events = self.store.list_events_for_session(&s.id)?;
        }
        self.metrics = report::build_report(&self.store, &self.workspace, 7).ok();
        Ok(())
    }

    fn selected_id(&self) -> Option<&str> {
        self.sessions.get(self.sel_session).map(|s| s.id.as_str())
    }
}

fn rel_time(now_ms: u64, ts_ms: u64) -> String {
    let diff = now_ms.saturating_sub(ts_ms) / 1000;
    match diff {
        0 => "just now".to_string(),
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s => format!("{}h", s / 3600),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
            let status = format!("{:?}", s.status);
            let color = theme::status_color(&status);
            let age = rel_time(now, s.started_at_ms);
            ListItem::new(Span::styled(
                format!("{:.16} {:.7} {age}", s.id, s.agent),
                Style::default().fg(color),
            ))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.sel_session));
    f.render_stateful_widget(
        ratatui::widgets::List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::DarkGray)),
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
    let border_color = if !app.left_focus {
        theme::BORDER_ACTIVE
    } else {
        theme::BORDER_INACTIVE
    };
    let block = Block::default()
        .title(format!("Events — {:.20}", id))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let now = now_ms();
    let items: Vec<ListItem> = app
        .events
        .iter()
        .map(|e| {
            let age = rel_time(now, e.ts_ms);
            let tool = e.tool.as_deref().unwrap_or("-");
            ListItem::new(format!("{age}  {kind:?}  {tool}", kind = e.kind))
        })
        .collect();
    f.render_widget(List::new(items).block(block), area);
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
    let text =
        format!("LIVE {pulse}  j/k move · Tab pane · m metrics · Enter detail · ? help · q quit");
    f.render_widget(Paragraph::new(text), area);
}

fn draw_help(f: &mut ratatui::Frame) {
    let text = "j/k ↑/↓  move  |  g/G top/bottom  |  Tab  switch pane\n\
                Enter  detail  |  Esc  back  |  r  refresh  |  q  quit";
    let block = Block::default().title("Help").borders(Borders::ALL);
    f.render_widget(Paragraph::new(text).block(block), f.area());
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
                        KeyCode::Char('q') | KeyCode::Esc if !app.detail => break,
                        KeyCode::Char('q') => { app.detail = false; app.show_help = false; }
                        KeyCode::Esc | KeyCode::Backspace => { app.detail = false; app.show_help = false; }
                        KeyCode::Char('?') => app.show_help = !app.show_help,
                        KeyCode::Char('m') => app.show_metrics = !app.show_metrics,
                        KeyCode::Tab => app.left_focus = !app.left_focus,
                        KeyCode::Char('r') => { let _ = app.refresh(); }
                        KeyCode::Char('j') | KeyCode::Down
                            if app.sel_session + 1 < app.sessions.len() =>
                        {
                            app.sel_session += 1;
                        }
                        KeyCode::Char('k') | KeyCode::Up if app.sel_session > 0 => {
                            app.sel_session -= 1;
                        }
                        KeyCode::Char('g') => app.sel_session = 0,
                        KeyCode::Char('G') => app.sel_session = app.sessions.len().saturating_sub(1),
                        KeyCode::Enter => app.detail = true,
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
