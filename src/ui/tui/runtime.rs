// SPDX-License-Identifier: AGPL-3.0-or-later

use super::app::App;
use super::background::{BackgroundWorkers, spawn_report_worker};
use super::input::{InputAction, handle_key};
use super::refresh::RefreshSchedule;
use super::render::draw;
use super::watch::{spawn_key_reader, spawn_wal_watcher, wait_for_deadline};
use anyhow::Result;
use arc_swap::ArcSwapOption;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::Stdout;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc;

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub async fn run(workspace: &Path) -> Result<()> {
    let workspace_buf = resolved_workspace_path(workspace);
    let workspace = workspace_buf.as_path();
    let db_path = crate::core::workspace::db_path(workspace)?;
    let metrics_cache = Arc::new(ArcSwapOption::from(None));
    let visualization_cache = Arc::new(ArcSwapOption::from(None));
    let report_dirty = Arc::new(AtomicBool::new(true));
    let report_notify = Arc::new(tokio::sync::Notify::new());
    let (report_ui_tx, mut report_ui_rx) = mpsc::unbounded_channel();
    let workers = BackgroundWorkers {
        report: spawn_report_worker(
            db_path.clone(),
            workspace.to_string_lossy().to_string(),
            metrics_cache.clone(),
            visualization_cache.clone(),
            report_dirty.clone(),
            report_notify.clone(),
            report_ui_tx,
        ),
    };
    let mut app = App::open(
        workspace,
        metrics_cache,
        visualization_cache,
        report_dirty,
        report_notify,
    )?;
    let mut terminal = open_terminal()?;
    let wal_dirty = Arc::new(AtomicBool::new(false));
    let (_watcher, mut wal_rx) = wal_channel(&db_path, wal_dirty.clone());
    let input_stop = Arc::new(AtomicBool::new(false));
    let mut key_rx = spawn_key_reader(input_stop.clone());
    let result = run_loop(
        &mut terminal,
        &mut app,
        wal_dirty,
        &mut wal_rx,
        &mut report_ui_rx,
        &mut key_rx,
    )
    .await;
    input_stop.store(true, Ordering::Release);
    workers.shutdown();
    drop(app);
    restore_terminal(&mut terminal)?;
    result
}

fn open_terminal() -> Result<TuiTerminal> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    std::panic::set_hook(Box::new(|_| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }));
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut TuiTerminal) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn wal_channel(
    db_path: &Path,
    dirty: Arc<AtomicBool>,
) -> (
    Option<notify::RecommendedWatcher>,
    mpsc::UnboundedReceiver<()>,
) {
    spawn_wal_watcher(&db_path.with_extension("db-wal"), dirty).map_or_else(
        |_| {
            let (_tx, rx) = mpsc::unbounded_channel();
            (None, rx)
        },
        |(watcher, rx)| (Some(watcher), rx),
    )
}

async fn run_loop(
    terminal: &mut TuiTerminal,
    app: &mut App,
    wal_dirty: Arc<AtomicBool>,
    wal_rx: &mut mpsc::UnboundedReceiver<()>,
    report_ui_rx: &mut mpsc::UnboundedReceiver<()>,
    key_rx: &mut mpsc::UnboundedReceiver<crossterm::event::KeyEvent>,
) -> Result<()> {
    let mut needs_draw = true;
    let mut refresh = RefreshSchedule::new();
    loop {
        draw_if_needed(terminal, app, &mut needs_draw)?;
        tokio::select! {
            Some(response) = app.store_rx.recv() => {
                app.apply_store_response(response);
                needs_draw = true;
            }
            _ = wait_for_deadline(refresh.deadline), if refresh.deadline.is_some() => {
                needs_draw |= refresh.on_deadline(app);
            }
            Some(_) = wal_rx.recv() => {
                needs_draw |= refresh.on_wal(app, &wal_dirty);
            }
            Some(_) = report_ui_rx.recv() => {
                app.sync_metrics_cache();
                needs_draw = true;
            }
            Some(key) = key_rx.recv() => match handle_key(app, key) {
                InputAction::None => {}
                InputAction::Draw => needs_draw = true,
                InputAction::Quit => break,
            },
            else => {}
        }
    }
    Ok(())
}

fn draw_if_needed(terminal: &mut TuiTerminal, app: &mut App, needs_draw: &mut bool) -> Result<()> {
    if !*needs_draw {
        return Ok(());
    }
    app.sync_metrics_cache();
    terminal.draw(|frame| {
        app.set_viewport_height(frame.area().height as usize);
        app.request_session_pages();
        app.request_event_pages();
        draw(frame, app);
    })?;
    *needs_draw = false;
    Ok(())
}

pub(super) fn resolved_workspace_path(workspace: &Path) -> PathBuf {
    crate::core::workspace::canonical(workspace)
}
