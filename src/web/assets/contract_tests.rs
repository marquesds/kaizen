// SPDX-License-Identifier: AGPL-3.0-or-later
use super::{INDEX, JS, SESSION_CONTROLS_JS, SNAPSHOT_STATE_JS};

#[test]
fn project_controls_live_in_header_navigation() {
    let header = INDEX.find("<header class=\"site-header\"").unwrap();
    let header_end = INDEX.find("</header>").unwrap();
    let controls = INDEX.find("class=\"project-controls\"").unwrap();
    assert!(header < controls && controls < header_end);
    assert!(INDEX.contains("<nav class=\"project-controls\" aria-label=\"Project controls\""));
    assert!(!INDEX.contains("<section class=\"project-controls\""));
}

#[test]
fn session_panel_has_one_described_search_field() {
    assert_eq!(INDEX.matches("type=\"search\"").count(), 1);
    assert!(INDEX.contains("<label for=\"session-search\">Search sessions</label>"));
    assert!(INDEX.contains("id=\"session-search\""));
    assert!(INDEX.contains("maxlength=\"256\""));
    assert!(INDEX.contains("aria-describedby=\"session-search-help\""));
    for term in ["prompt", "ID", "agent", "model", "status", "branch", "tool"] {
        assert!(INDEX.contains(term), "search help missing {term}");
    }
}

#[test]
fn session_panel_exposes_native_pagination_and_live_status() {
    for needle in [
        "id=\"session-previous\"",
        "id=\"session-next\"",
        "id=\"session-page-status\"",
        "id=\"session-count-note\" aria-live=\"polite\"",
    ] {
        assert!(INDEX.contains(needle), "index missing {needle}");
    }
    assert!(INDEX.contains("<button id=\"session-previous\" type=\"button\""));
    assert!(INDEX.contains("<button id=\"session-next\" type=\"button\""));
}

#[test]
fn snapshot_requests_include_search_and_offset() {
    assert!(JS.contains("q: state.query"));
    assert!(JS.contains("offset: state.offset"));
    assert!(JS.contains("state.selected = \"\""));
    assert!(SESSION_CONTROLS_JS.contains("DEBOUNCE_MS = 250"));
    assert!(SESSION_CONTROLS_JS.contains("{ query, offset: 0 }"));
    assert!(SESSION_CONTROLS_JS.contains("searchQuery(), offset"));
    assert!(SNAPSHOT_STATE_JS.contains("fallbackOffset"));
}
