// SPDX-License-Identifier: AGPL-3.0-or-later

use super::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub(super) enum InputAction {
    None,
    Draw,
    Quit,
}

pub(super) fn handle_key(app: &mut App, key: KeyEvent) -> InputAction {
    if app.filter_mode {
        handle_filter_key(app, key.code)
    } else {
        handle_normal_key(app, key.code)
    }
}

fn handle_filter_key(app: &mut App, code: KeyCode) -> InputAction {
    match code {
        KeyCode::Enter => apply_filter(app),
        KeyCode::Esc => cancel_filter(app),
        KeyCode::Backspace => {
            app.filter_buf.pop();
            InputAction::Draw
        }
        KeyCode::Char(character) => {
            app.filter_buf.push(character);
            InputAction::Draw
        }
        _ => InputAction::None,
    }
}

fn apply_filter(app: &mut App) -> InputAction {
    app.agent_filter = app.filter_buf.trim().to_string();
    app.filter_mode = false;
    let _ = app.refresh_full();
    InputAction::Draw
}

fn cancel_filter(app: &mut App) -> InputAction {
    app.filter_mode = false;
    app.filter_buf.clear();
    InputAction::Draw
}

fn handle_normal_key(app: &mut App, code: KeyCode) -> InputAction {
    match code {
        KeyCode::Char('/') => start_filter(app),
        KeyCode::Char('y') if app.left_focus => copy_session_id(app),
        KeyCode::Char('q') | KeyCode::Esc if !app.detail && !app.show_help => InputAction::Quit,
        KeyCode::Char('q') if app.show_help => close_help(app),
        KeyCode::Char('q') => close_overlay(app),
        KeyCode::Esc | KeyCode::Backspace => close_overlay(app),
        KeyCode::Char('?') => toggle_help(app),
        KeyCode::Char('m') => toggle_metrics(app),
        KeyCode::Tab => switch_pane(app),
        KeyCode::Char('r') => refresh(app),
        KeyCode::Char('j') | KeyCode::Down => move_cursor(app, 1),
        KeyCode::Char('k') | KeyCode::Up => move_cursor(app, -1),
        KeyCode::Char('g') => first(app),
        KeyCode::Char('G') => last(app),
        KeyCode::Enter if app.selected_event().is_some() && !app.show_metrics => {
            app.detail = !app.detail;
            InputAction::Draw
        }
        _ => InputAction::None,
    }
}

fn start_filter(app: &mut App) -> InputAction {
    app.filter_mode = true;
    app.filter_buf.clone_from(&app.agent_filter);
    InputAction::Draw
}

fn copy_session_id(app: &mut App) -> InputAction {
    let Some(id) = app.selected_id().map(str::to_string) else {
        return InputAction::None;
    };
    app.clipboard_note = clipboard_note(&id);
    InputAction::Draw
}

fn clipboard_note(id: &str) -> String {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return "no clipboard".to_string();
    };
    if clipboard.set_text(id).is_ok() {
        "copied session id".to_string()
    } else {
        "clipboard write failed".to_string()
    }
}

fn close_help(app: &mut App) -> InputAction {
    app.show_help = false;
    InputAction::Draw
}

fn close_overlay(app: &mut App) -> InputAction {
    app.detail = false;
    app.show_help = false;
    InputAction::Draw
}

fn toggle_help(app: &mut App) -> InputAction {
    app.show_help = !app.show_help;
    InputAction::Draw
}

fn toggle_metrics(app: &mut App) -> InputAction {
    app.show_metrics = !app.show_metrics;
    InputAction::Draw
}

fn switch_pane(app: &mut App) -> InputAction {
    app.left_focus = !app.left_focus;
    InputAction::Draw
}

fn refresh(app: &mut App) -> InputAction {
    let _ = app.refresh_full();
    InputAction::Draw
}

fn move_cursor(app: &mut App, delta: isize) -> InputAction {
    if app.show_metrics || app.left_focus {
        app.sessions.move_by(delta);
        app.after_session_cursor_move();
    } else {
        app.events.move_by(delta);
        app.request_event_pages();
    }
    InputAction::Draw
}

fn first(app: &mut App) -> InputAction {
    if app.show_metrics || app.left_focus {
        app.sessions.cursor = 0;
        app.after_session_cursor_move();
    } else {
        app.events.cursor = 0;
        app.request_event_pages();
    }
    InputAction::Draw
}

fn last(app: &mut App) -> InputAction {
    if app.show_metrics || app.left_focus {
        app.sessions.jump_last();
        app.after_session_cursor_move();
    } else {
        app.events.jump_last_loaded();
        app.request_event_pages();
    }
    InputAction::Draw
}
