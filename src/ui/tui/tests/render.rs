// SPDX-License-Identifier: AGPL-3.0-or-later

use super::super::{
    app::App,
    format::{event_detail_text, time_ago_label},
    render::draw,
    runtime,
    worker::StoreResponse,
};
use super::state::{event, session};
use crate::store::SessionPage;

#[test]
fn store_errors_are_visible() {
    let mut app = App::test();
    app.error_note = "store unavailable".to_string();
    assert!(screen(&app).contains("Store error: store unavailable"));
}

#[test]
fn store_errors_remain_visible_in_help() {
    let mut app = App::test();
    app.error_note = "store unavailable".to_string();
    app.show_help = true;
    assert!(screen(&app).contains("Store error: store unavailable"));
}

#[test]
fn feedback_store_errors_are_visible() {
    let mut app = App::test();
    app.apply_store_response(StoreResponse::Feedback {
        token: app.feedback_token,
        result: Err("feedback unavailable".to_string()),
    });
    assert!(screen(&app).contains("feedback unavailable"));
}

#[test]
fn empty_sessions_are_explicit() {
    let mut app = App::test();
    apply_sessions(&mut app, Vec::new(), 0);
    assert!(screen(&app).contains("No sessions found"));
}

#[test]
fn partial_session_load_is_labeled() {
    let mut app = App::test();
    apply_sessions(&mut app, vec![session("session-1", 10)], 30);
    assert!(screen(&app).contains("1/30 loaded"));
}

#[test]
fn help_keeps_keyboard_parity() {
    let mut app = App::test();
    app.show_help = true;
    let screen = screen(&app);
    assert!(screen.contains("m  metrics"));
    assert!(screen.contains("?  close help"));
}

#[test]
fn event_details_use_plain_labels() {
    let mut event = event("session-1", 7);
    event.tokens_in = Some(3);
    event.cost_usd_e6 = Some(25_000);
    let text = event_detail_text(&event, &Default::default());
    assert!(text.contains("Event 7"));
    assert!(text.contains("Type: tool call"));
    assert!(text.contains("Input tokens: 3"));
    assert!(!text.contains("cost_e6"));
}

fn screen(app: &App) -> String {
    let backend = ratatui::backend::TestBackend::new(140, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|frame| draw(frame, app)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

fn apply_sessions(app: &mut App, rows: Vec<crate::core::event::SessionRecord>, total: usize) {
    app.apply_store_response(StoreResponse::SessionsPage {
        token: app.sessions.generation(),
        offset: 0,
        result: Ok(SessionPage {
            rows,
            total,
            next_offset: (total > 0).then_some(1),
        }),
    });
}

#[test]
fn time_ago_just_now() {
    assert_eq!(time_ago_label(10_000, 10_000), "just now");
}

#[test]
fn time_ago_treats_small_ts_as_seconds() {
    let now = 1_700_000_000_000u64;
    let label = time_ago_label(now, 1_700_000_000u64);
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
fn runtime_shutdown_timeout_bounds_blocking_task() {
    let start = std::time::Instant::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.spawn_blocking(|| std::thread::sleep(std::time::Duration::from_millis(200)));
    runtime.shutdown_timeout(std::time::Duration::from_millis(10));
    assert!(start.elapsed() < std::time::Duration::from_millis(150));
}

#[cfg(unix)]
#[test]
fn resolved_workspace_path_follows_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let real = temp.path().join("real");
    let link = temp.path().join("link");
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(
        runtime::resolved_workspace_path(&link),
        std::fs::canonicalize(real).unwrap()
    );
}
