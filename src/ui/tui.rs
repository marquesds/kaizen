// SPDX-License-Identifier: AGPL-3.0-or-later
//! Two-pane TUI: session list (left) + events (right).

mod view;
mod worker;

use crate::core::event::{Event, SessionRecord, SessionStatus};
use crate::metrics::types::MetricsReport;
use crate::metrics::{index, report};
use crate::store::span_tree::SpanNode;
use crate::store::{SessionFilter, Store};
use crate::ui::theme;
use anyhow::Result;
use arc_swap::ArcSwapOption;
use crossterm::{
    event::{self as cxev, Event as CxEvent, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use notify::{EventKind as NotifyEventKind, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use time::OffsetDateTime;
use tokio::sync::{Notify, mpsc};
use tokio::time::{Duration, Instant, sleep_until};
use view::{DetailData, DetailState, EventView, SessionView};
use worker::{StoreRequest, StoreResponse, spawn_store_worker};

const THIRTY_DAYS_SEC: u64 = 30 * 24 * 3600;
/// Timestamps below this are treated as Unix **seconds** when `now` looks like ms (ingest mistakes).
const MS_HEURISTIC_THRESHOLD: u64 = 1_000_000_000_000;
const WAL_REFRESH_COALESCE_MS: u64 = 100;
const REPORT_REFRESH_MIN_MS: u64 = 2_000;
const DEFAULT_VIEWPORT_HEIGHT: usize = 32;

struct App {
    sessions: SessionView,
    events: EventView,
    detail_state: DetailState,
    agent_filter: String,
    filter_mode: bool,
    filter_buf: String,
    clipboard_note: String,
    left_focus: bool,
    show_help: bool,
    detail: bool,
    show_metrics: bool,
    metrics: Option<MetricsReport>,
    metrics_cache: Arc<ArcSwapOption<MetricsReport>>,
    report_dirty: Arc<AtomicBool>,
    report_notify: Arc<Notify>,
    pulse: bool,
    workspace: String,
    store_tx: mpsc::UnboundedSender<StoreRequest>,
    store_rx: mpsc::UnboundedReceiver<StoreResponse>,
    feedback_scores: HashMap<String, u8>,
    feedback_token: u64,
    detail_token: u64,
    last_session_id: Option<String>,
    session_viewport_height: usize,
    event_viewport_height: usize,
    error_note: String,
}

impl App {
    fn open(
        workspace: &Path,
        metrics_cache: Arc<ArcSwapOption<MetricsReport>>,
        report_dirty: Arc<AtomicBool>,
        report_notify: Arc<Notify>,
    ) -> Result<Self> {
        let db = crate::core::workspace::db_path(workspace)?;
        Store::open(&db)?;
        let (store_tx, store_rx) = spawn_store_worker(db);
        let ws = workspace.to_string_lossy().to_string();
        let metrics = metrics_cache.load_full().as_deref().cloned();
        let app = Self {
            sessions: SessionView::new(),
            events: EventView::new(),
            detail_state: DetailState::Idle,
            agent_filter: String::new(),
            filter_mode: false,
            filter_buf: String::new(),
            clipboard_note: String::new(),
            left_focus: true,
            show_help: false,
            detail: false,
            show_metrics: false,
            metrics,
            metrics_cache,
            report_dirty,
            report_notify,
            pulse: false,
            workspace: ws,
            store_tx,
            store_rx,
            feedback_scores: HashMap::new(),
            feedback_token: 0,
            detail_token: 0,
            last_session_id: None,
            session_viewport_height: DEFAULT_VIEWPORT_HEIGHT,
            event_viewport_height: DEFAULT_VIEWPORT_HEIGHT,
            error_note: String::new(),
        };
        app.mark_report_dirty();
        let mut app = app;
        app.request_session_pages();
        app.request_feedback_for_viewport();
        Ok(app)
    }

    fn sync_metrics_cache(&mut self) {
        self.metrics = self.metrics_cache.load_full().as_deref().cloned();
    }

    fn mark_report_dirty(&self) {
        self.report_dirty.store(true, Ordering::Release);
        self.report_notify.notify_one();
    }

    fn refresh_full(&mut self) -> Result<()> {
        self.sessions.reset();
        self.events.clear();
        self.detail_state = DetailState::Idle;
        self.last_session_id = None;
        self.pulse = !self.pulse;
        self.request_session_pages();
        self.sync_metrics_cache();
        self.mark_report_dirty();
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        self.pulse = !self.pulse;
        self.request_session_pages();
        self.request_selected_detail();
        self.request_event_pages();
        self.sync_metrics_cache();
        self.mark_report_dirty();
        Ok(())
    }

    fn filter(&self) -> SessionFilter {
        SessionFilter {
            agent_prefix: Some(self.agent_filter.trim().to_lowercase()).filter(|s| !s.is_empty()),
            status: None,
            since_ms: None,
        }
    }

    fn request_session_pages(&mut self) {
        let offsets = self
            .sessions
            .needed_page_offsets(self.session_viewport_height);
        for offset in offsets {
            if self.sessions.request_page(offset) {
                let _ = self.store_tx.send(StoreRequest::SessionsPage {
                    token: self.sessions.generation(),
                    workspace: self.workspace.clone(),
                    offset,
                    limit: self.sessions.page_size,
                    filter: self.filter(),
                });
            }
        }
    }

    fn request_feedback_for_viewport(&mut self) {
        let ids: Vec<String> = self
            .sessions
            .visible_rows(self.session_viewport_height)
            .into_iter()
            .filter_map(|(_, row)| row.map(|s| s.id.clone()))
            .filter(|id| !self.feedback_scores.contains_key(id))
            .collect();
        if ids.is_empty() {
            return;
        }
        self.feedback_token = self.feedback_token.wrapping_add(1);
        let _ = self.store_tx.send(StoreRequest::Feedback {
            token: self.feedback_token,
            ids,
        });
    }

    fn request_selected_detail(&mut self) {
        let Some(id) = self.selected_id().map(str::to_string) else {
            self.detail_state = DetailState::Idle;
            self.events.clear();
            self.last_session_id = None;
            return;
        };
        if self.last_session_id.as_deref() != Some(&id) {
            self.events.reset_for(&id);
            self.detail_token = self.detail_token.wrapping_add(1);
            self.detail_state = DetailState::Loading {
                token: self.detail_token,
                session_id: id.clone(),
            };
            let _ = self.store_tx.send(StoreRequest::Detail {
                token: self.detail_token,
                session_id: id.clone(),
            });
            self.last_session_id = Some(id);
        }
        self.request_event_pages();
    }

    fn request_event_pages(&mut self) {
        let Some(session_id) = self.events.session_id().map(str::to_string) else {
            return;
        };
        for after_seq in self.events.needed_after_seq(self.event_viewport_height) {
            if self.events.request_page(after_seq) {
                let _ = self.store_tx.send(StoreRequest::EventsPage {
                    token: self.events.generation(),
                    session_id: session_id.clone(),
                    after_seq,
                    limit: self.events.page_size,
                });
            }
        }
    }

    fn apply_store_response(&mut self, response: StoreResponse) {
        match response {
            StoreResponse::SessionsPage {
                token,
                offset,
                result,
            } => self.apply_session_page(token, offset, result),
            StoreResponse::EventsPage {
                token,
                session_id,
                after_seq,
                result,
            } => self.apply_event_page(token, &session_id, after_seq, result),
            StoreResponse::Detail {
                token,
                session_id,
                result,
            } => self.apply_detail(token, &session_id, result),
            StoreResponse::Feedback { token, result } => self.apply_feedback(token, result),
        }
    }

    fn apply_session_page(
        &mut self,
        token: u64,
        offset: usize,
        result: Result<crate::store::SessionPage, String>,
    ) {
        if token != self.sessions.generation() {
            return;
        }
        match result {
            Ok(page) => {
                self.sessions.finish_page(offset, page.rows, page.total);
                self.error_note.clear();
                self.request_feedback_for_viewport();
                self.request_selected_detail();
            }
            Err(err) => {
                self.sessions.finish_error(offset);
                self.error_note = err;
            }
        }
    }

    fn apply_event_page(
        &mut self,
        token: u64,
        session_id: &str,
        after_seq: u64,
        result: Result<Vec<Event>, String>,
    ) {
        if token != self.events.generation() || self.events.session_id() != Some(session_id) {
            return;
        }
        match result {
            Ok(rows) => self.events.finish_page(after_seq, rows),
            Err(err) => {
                self.events.finish_error(after_seq);
                self.error_note = err;
            }
        }
    }

    fn apply_detail(&mut self, token: u64, session_id: &str, result: Result<DetailData, String>) {
        match &self.detail_state {
            DetailState::Loading {
                token: active,
                session_id: active_id,
            } if *active == token && active_id == session_id => {}
            _ => return,
        }
        self.detail_state = match result {
            Ok(data) => DetailState::Ready(data),
            Err(err) => DetailState::Error(err),
        };
    }

    fn apply_feedback(&mut self, token: u64, result: Result<HashMap<String, u8>, String>) {
        if token != self.feedback_token {
            return;
        }
        if let Ok(scores) = result {
            self.feedback_scores.extend(scores);
        }
    }

    fn set_viewport_height(&mut self, height: usize) {
        let h = height.saturating_sub(4);
        self.session_viewport_height = h.max(1);
        self.event_viewport_height = h.max(1);
        self.sessions
            .set_viewport_height(self.session_viewport_height);
        self.events.set_viewport_height(self.event_viewport_height);
    }

    fn after_session_cursor_move(&mut self) {
        self.request_session_pages();
        self.request_feedback_for_viewport();
        self.request_selected_detail();
    }

    fn selected_session(&self) -> Option<&SessionRecord> {
        self.sessions.selected()
    }

    fn selected_id(&self) -> Option<&str> {
        self.selected_session().map(|s| s.id.as_str())
    }

    fn selected_event(&self) -> Option<&Event> {
        self.events.selected()
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
    let title = if app.agent_filter.is_empty() {
        format!("Sessions ({})", app.sessions.total)
    } else {
        format!(
            "Sessions {} (agent prefix: {:?})",
            app.sessions.total, app.agent_filter
        )
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let now = now_ms();
    let items: Vec<ListItem> = app
        .sessions
        .visible_rows(area.height.saturating_sub(2) as usize)
        .into_iter()
        .map(|(idx, row)| {
            row.map(|s| session_item(now, s, &app.feedback_scores))
                .unwrap_or_else(|| ListItem::new(format!("{idx:>6}  loading...")))
        })
        .collect();
    let mut state = ListState::default();
    state.select(
        app.sessions
            .selected_local_index(area.height.saturating_sub(2) as usize),
    );
    f.render_stateful_widget(
        ratatui::widgets::List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
        area,
        &mut state,
    );
}

fn session_item<'a>(
    now: u64,
    s: &'a SessionRecord,
    feedback_scores: &HashMap<String, u8>,
) -> ListItem<'a> {
    let st = format!("{:?}", s.status);
    let st_color = theme::status_color(&st);
    let age = time_ago_label(now, s.started_at_ms);
    let tag = session_status_letter(s);
    let m = model_suffix(&s.model);
    let score_span = feedback_scores.get(&s.id).map(|&sc| {
        let color = match sc {
            1..=2 => Color::Red,
            3 => Color::Yellow,
            _ => Color::Green,
        };
        Span::styled(format!("★{sc}"), Style::default().fg(color))
    });
    let mut spans = vec![
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
    ];
    if let Some(s) = score_span {
        spans.push(Span::raw(" "));
        spans.push(s);
    }
    ListItem::new(Line::from(spans))
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
    let leads = app.detail_state.leads();
    let spans = app.detail_state.spans();
    if app.detail
        && let Some(ev) = app.selected_event()
    {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(2), Constraint::Length(10)])
            .split(area);
        render_event_list(f, app, split[0], title.clone(), border_color, now);
        let detail = event_detail_text(ev, leads);
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
    if !spans.is_empty() {
        let max_depth: u32 = spans.iter().map(|n| n.span.depth).max().unwrap_or(0);
        let strip_h = (max_depth + 3).min(8) as u16;
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(2), Constraint::Length(strip_h)])
            .split(area);
        render_event_list(f, app, split[0], title, border_color, now);
        let span_text: Vec<Line> = span_depth_lines(spans);
        let span_block = Block::default()
            .title("Span tree")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme::BORDER_INACTIVE));
        f.render_widget(
            Paragraph::new(span_text)
                .block(span_block)
                .wrap(Wrap { trim: false }),
            split[1],
        );
        return;
    }
    if matches!(app.detail_state, DetailState::Loading { .. }) && app.events.total_loaded == 0 {
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        f.render_widget(Paragraph::new("loading...").block(block), area);
        return;
    }
    if let Some(err) = app.detail_state.error_message()
        && app.events.total_loaded == 0
    {
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        f.render_widget(Paragraph::new(err.to_string()).block(block), area);
        return;
    }
    render_event_list(f, app, area, title, border_color, now);
}

fn render_event_list(
    f: &mut ratatui::Frame,
    app: &App,
    area: ratatui::layout::Rect,
    title: String,
    border_color: Color,
    now: u64,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let items: Vec<ListItem> = app
        .events
        .visible_rows(area.height.saturating_sub(2) as usize)
        .into_iter()
        .map(|(idx, row)| {
            row.map(|e| ListItem::new(event_row_text(now, e, app.detail_state.leads())))
                .unwrap_or_else(|| ListItem::new(format!("{idx:>6}  loading...")))
        })
        .collect();
    let mut state = ListState::default();
    state.select(
        app.events
            .selected_local_index(area.height.saturating_sub(2) as usize),
    );
    f.render_stateful_widget(
        List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White)),
        area,
        &mut state,
    );
}

fn span_depth_lines(nodes: &[SpanNode]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for n in nodes {
        push_span_line(&mut lines, n, 0);
    }
    lines
}

fn push_span_line(lines: &mut Vec<Line<'static>>, node: &SpanNode, depth: u32) {
    let indent = "  ".repeat(depth as usize);
    let prefix = if depth == 0 { "┌ " } else { "├ " };
    let cost = node
        .span
        .subtree_cost_usd_e6
        .map(|c| format!(" ${:.4}", c as f64 / 1_000_000.0))
        .unwrap_or_default();
    let text = format!("{}{}{}{}", indent, prefix, node.span.tool, cost);
    lines.push(Line::from(Span::raw(text)));
    for child in &node.children {
        push_span_line(lines, child, depth + 1);
    }
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
    let empty = app.metrics.is_none()
        || app
            .metrics
            .as_ref()
            .is_some_and(|m| m.slowest_tools.is_empty() && m.hottest_files.is_empty());
    let text = if empty {
        "(No metrics in this window yet. Run `kaizen metrics` in a shell, or `r` here after a repo is indexed.)\n\nMetrics need a successful snapshot + events for tool spans — see docs/telemetry-journey.md."
            .to_string()
    } else {
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
        lines.join("\n")
    };
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_statusbar(f: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let pulse = if app.pulse { "●" } else { "○" };
    let text = if app.filter_mode {
        format!(
            "FILTER  type agent substring  |  Enter apply  |  Esc cancel  |  buffer: {}",
            app.filter_buf
        )
    } else {
        let note = if app.clipboard_note.is_empty() {
            String::new()
        } else {
            format!("  |  {}", app.clipboard_note)
        };
        format!(
            "LIVE {pulse}  j/k  Tab  m metrics  / filter  y copy id  Enter detail  ? help  q quit{note}"
        )
    };
    f.render_widget(Paragraph::new(text), area);
}

fn draw_help(f: &mut ratatui::Frame) {
    let text = "j/k ↑/↓  move in focused pane  |  g/G first/last  |  Tab  switch pane\n\
                Enter  toggle event detail  |  Esc  back  |  r  refresh  |  q  quit\n\
                /  filter sessions by agent substring  |  y  copy session id (left pane)";
    let block = Block::default().title("Help").borders(Borders::ALL);
    f.render_widget(Paragraph::new(text).block(block), f.area());
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn spawn_key_reader(stop: Arc<AtomicBool>) -> mpsc::UnboundedReceiver<KeyEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::task::spawn_blocking(move || {
        while !stop.load(Ordering::Acquire) {
            match cxev::poll(Duration::from_millis(250)) {
                Ok(true) => match cxev::read() {
                    Ok(CxEvent::Key(k)) if k.kind == KeyEventKind::Press => {
                        if tx.send(k).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                },
                Ok(false) => {}
                Err(_) => break,
            }
        }
    });
    rx
}

fn spawn_wal_watcher(
    wal_path: &Path,
    dirty: Arc<AtomicBool>,
) -> Result<(RecommendedWatcher, mpsc::UnboundedReceiver<()>)> {
    let (tx, rx) = mpsc::unbounded_channel();
    let watched_wal = wal_path.to_path_buf();
    let callback_wal = watched_wal.clone();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res
                && !matches!(event.kind, NotifyEventKind::Access(_))
                && event.paths.iter().any(|p| p == &callback_wal)
            {
                dirty.store(true, Ordering::Release);
                let _ = tx.send(());
            }
        },
        notify::Config::default(),
    )?;
    watcher.watch(
        watched_wal.parent().unwrap_or_else(|| Path::new(".")),
        RecursiveMode::NonRecursive,
    )?;
    Ok((watcher, rx))
}

fn spawn_report_worker(
    db_path: PathBuf,
    workspace: String,
    cache: Arc<ArcSwapOption<MetricsReport>>,
    dirty: Arc<AtomicBool>,
    notify: Arc<Notify>,
    ui_tx: mpsc::UnboundedSender<()>,
) {
    tokio::spawn(async move {
        let mut last_run = Instant::now() - Duration::from_millis(REPORT_REFRESH_MIN_MS);
        loop {
            notify.notified().await;
            let ready_at = last_run + Duration::from_millis(REPORT_REFRESH_MIN_MS);
            let now = Instant::now();
            if now < ready_at {
                sleep_until(ready_at).await;
            }
            if !dirty.swap(false, Ordering::AcqRel) {
                continue;
            }
            let db = db_path.clone();
            let ws = workspace.clone();
            let next = tokio::task::spawn_blocking(move || {
                let store = Store::open_read_only(&db)?;
                report::build_report(&store, &ws, 7)
            })
            .await;
            last_run = Instant::now();
            if let Ok(Ok(report)) = next {
                cache.store(Some(Arc::new(report)));
                let _ = ui_tx.send(());
            }
        }
    });
}

fn spawn_tui_setup_worker(
    workspace: PathBuf,
    db_path: PathBuf,
    report_dirty: Arc<AtomicBool>,
    report_notify: Arc<Notify>,
) {
    tokio::task::spawn_blocking(move || {
        if let Ok(store) = Store::open(&db_path) {
            let _ = index::ensure_indexed(&store, &workspace, false);
            report_dirty.store(true, Ordering::Release);
            report_notify.notify_one();
        }
    });
}

async fn wait_for_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

/// Entry point. Opens terminal, refreshes on WAL changes, handles keys.
pub async fn run(workspace: &Path) -> Result<()> {
    let workspace_buf = resolved_workspace_path(workspace);
    let workspace = workspace_buf.as_path();
    let db_path = crate::core::workspace::db_path(workspace)?;
    let metrics_cache = Arc::new(ArcSwapOption::from(None));
    let report_dirty = Arc::new(AtomicBool::new(true));
    let report_notify = Arc::new(Notify::new());
    let (report_ui_tx, mut report_ui_rx) = mpsc::unbounded_channel();
    spawn_report_worker(
        db_path.clone(),
        workspace.to_string_lossy().to_string(),
        metrics_cache.clone(),
        report_dirty.clone(),
        report_notify.clone(),
        report_ui_tx,
    );
    let mut app = App::open(
        workspace,
        metrics_cache,
        report_dirty.clone(),
        report_notify.clone(),
    )?;
    spawn_tui_setup_worker(
        workspace.to_path_buf(),
        db_path.clone(),
        report_dirty.clone(),
        report_notify.clone(),
    );
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    std::panic::set_hook(Box::new(|_| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }));

    let wal_dirty = Arc::new(AtomicBool::new(false));
    let (_watcher, mut wal_rx) =
        match spawn_wal_watcher(&db_path.with_extension("db-wal"), wal_dirty.clone()) {
            Ok((watcher, rx)) => (Some(watcher), rx),
            Err(_) => {
                let (_tx, rx) = mpsc::unbounded_channel();
                (None, rx)
            }
        };
    let input_stop = Arc::new(AtomicBool::new(false));
    let mut key_rx = spawn_key_reader(input_stop.clone());
    let mut needs_draw = true;
    let mut pending_refresh = false;
    let mut refresh_deadline = None;
    let mut last_refresh = Instant::now() - Duration::from_millis(WAL_REFRESH_COALESCE_MS);

    loop {
        if needs_draw {
            app.sync_metrics_cache();
            terminal.draw(|f| {
                app.set_viewport_height(f.area().height as usize);
                app.request_session_pages();
                app.request_event_pages();
                draw(f, &app);
            })?;
            needs_draw = false;
        }
        tokio::select! {
            Some(response) = app.store_rx.recv() => {
                app.apply_store_response(response);
                needs_draw = true;
            }
            _ = wait_for_deadline(refresh_deadline), if refresh_deadline.is_some() => {
                refresh_deadline = None;
                if pending_refresh {
                    pending_refresh = false;
                    if app.refresh().is_ok() {
                        last_refresh = Instant::now();
                        needs_draw = true;
                    }
                }
            }
            Some(_) = wal_rx.recv() => {
                if wal_dirty.swap(false, Ordering::AcqRel) {
                    let ready_at = last_refresh + Duration::from_millis(WAL_REFRESH_COALESCE_MS);
                    let now = Instant::now();
                    if now >= ready_at {
                        if app.refresh().is_ok() {
                            last_refresh = now;
                            needs_draw = true;
                        }
                    } else {
                        pending_refresh = true;
                        refresh_deadline = Some(ready_at);
                    }
                }
            }
            Some(_) = report_ui_rx.recv() => {
                app.sync_metrics_cache();
                needs_draw = true;
            }
            Some(k) = key_rx.recv() => {
                    if app.filter_mode {
                        match k.code {
                            KeyCode::Enter => {
                                app.agent_filter = app.filter_buf.trim().to_string();
                                app.filter_mode = false;
                                let _ = app.refresh_full();
                                needs_draw = true;
                            }
                            KeyCode::Esc => {
                                app.filter_mode = false;
                                app.filter_buf.clear();
                                needs_draw = true;
                            }
                            KeyCode::Backspace => {
                                app.filter_buf.pop();
                                needs_draw = true;
                            }
                            KeyCode::Char(c) => {
                                app.filter_buf.push(c);
                                needs_draw = true;
                            }
                            _ => {}
                        }
                        continue;
                    }
                    match k.code {
                        KeyCode::Char('/') => {
                            app.filter_mode = true;
                            app.filter_buf.clone_from(&app.agent_filter);
                            needs_draw = true;
                        }
                        KeyCode::Char('y') if app.left_focus => {
                            if let Some(id) = app.selected_id() {
                                match arboard::Clipboard::new() {
                                    Ok(mut cb) => {
                                        if cb.set_text(id).is_ok() {
                                            app.clipboard_note = "copied session id".to_string();
                                        } else {
                                            app.clipboard_note = "clipboard write failed".to_string();
                                        }
                                    }
                                    Err(_) => app.clipboard_note = "no clipboard".to_string(),
                                }
                                needs_draw = true;
                            }
                        }
                        KeyCode::Char('q') | KeyCode::Esc if !app.detail && !app.show_help => break,
                        KeyCode::Char('q') if app.show_help => { app.show_help = false; needs_draw = true; }
                        KeyCode::Char('q') => { app.detail = false; app.show_help = false; needs_draw = true; }
                        KeyCode::Esc | KeyCode::Backspace => {
                            app.detail = false;
                            app.show_help = false;
                            needs_draw = true;
                        }
                        KeyCode::Char('?') => { app.show_help = !app.show_help; needs_draw = true; }
                        KeyCode::Char('m') => { app.show_metrics = !app.show_metrics; needs_draw = true; }
                        KeyCode::Tab => {
                            app.left_focus = !app.left_focus;
                            needs_draw = true;
                        }
                        KeyCode::Char('r') => { let _ = app.refresh_full(); needs_draw = true; }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if app.show_metrics || app.left_focus {
                                app.sessions.move_by(1);
                                app.after_session_cursor_move();
                                needs_draw = true;
                            } else {
                                app.events.move_by(1);
                                app.request_event_pages();
                                needs_draw = true;
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if app.show_metrics || app.left_focus {
                                app.sessions.move_by(-1);
                                app.after_session_cursor_move();
                                needs_draw = true;
                            } else {
                                app.events.move_by(-1);
                                app.request_event_pages();
                                needs_draw = true;
                            }
                        }
                        KeyCode::Char('g') => {
                            if app.show_metrics || app.left_focus {
                                app.sessions.cursor = 0;
                                app.after_session_cursor_move();
                            } else {
                                app.events.cursor = 0;
                                app.request_event_pages();
                            }
                            needs_draw = true;
                        }
                        KeyCode::Char('G') => {
                            if app.show_metrics || app.left_focus {
                                app.sessions.jump_last();
                                app.after_session_cursor_move();
                            } else {
                                app.events.jump_last_loaded();
                                app.request_event_pages();
                            }
                            needs_draw = true;
                        }
                        KeyCode::Enter if app.selected_event().is_some() && !app.show_metrics => {
                            app.detail = !app.detail;
                            needs_draw = true;
                        }
                        _ => {}
                    }
            }
            else => {}
        }
    }

    input_stop.store(true, Ordering::Release);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn resolved_workspace_path(workspace: &Path) -> PathBuf {
    crate::core::workspace::canonical(workspace)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{EventKind, EventSource};

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

    #[test]
    fn session_view_clamps_cursor_and_suppresses_double_load() {
        let mut view = view::SessionView::new();
        view.set_viewport_height(4);
        assert_eq!(view.needed_page_offsets(4), vec![0]);
        assert!(view.request_page(0));
        assert!(!view.request_page(0));
        view.finish_page(0, vec![session("a", 3), session("b", 2)], 2);
        view.move_by(99);
        assert_eq!(view.cursor, 1);
        assert_eq!(view.selected().unwrap().id, "b");
    }

    #[test]
    fn session_view_eviction_keeps_cursor_page() {
        let mut view = view::SessionView::new();
        view.page_size = 2;
        for offset in (0..=20).step_by(2) {
            view.cursor = offset;
            view.finish_page(
                offset,
                vec![session(&format!("s{offset}"), offset as u64)],
                30,
            );
        }
        assert!(view.window.contains_key(&20));
        assert!(!view.window.contains_key(&0));
    }

    #[test]
    fn event_view_paginates_from_zero_and_resets_generation() {
        let mut view = view::EventView::new();
        view.page_size = 2;
        view.reset_for("s1");
        let token = view.generation();
        assert!(view.needed_after_seq(4).starts_with(&[0, 2]));
        assert!(view.request_page(0));
        assert!(!view.request_page(0));
        view.finish_page(0, vec![event("s1", 0), event("s1", 1)]);
        view.reset_for("s2");
        assert_ne!(view.generation(), token);
        assert!(view.window.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn resolved_workspace_path_follows_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        let link = tmp.path().join("link");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(
            resolved_workspace_path(&link),
            std::fs::canonicalize(real).unwrap()
        );
    }

    fn session(id: &str, started_at_ms: u64) -> SessionRecord {
        SessionRecord {
            id: id.to_string(),
            agent: "cursor".to_string(),
            model: None,
            workspace: "/ws".to_string(),
            started_at_ms,
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: "/trace".to_string(),
            start_commit: None,
            end_commit: None,
            branch: None,
            dirty_start: None,
            dirty_end: None,
            repo_binding_source: None,
            prompt_fingerprint: None,
            parent_session_id: None,
            agent_version: None,
            os: None,
            arch: None,
            repo_file_count: None,
            repo_total_loc: None,
        }
    }

    fn event(session_id: &str, seq: u64) -> Event {
        Event {
            session_id: session_id.to_string(),
            seq,
            ts_ms: seq,
            ts_exact: true,
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool: None,
            tool_call_id: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            stop_reason: None,
            latency_ms: None,
            ttft_ms: None,
            retry_count: None,
            context_used_tokens: None,
            context_max_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            system_prompt_tokens: None,
            payload: serde_json::Value::Null,
        }
    }
}
