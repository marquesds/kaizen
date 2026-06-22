// SPDX-License-Identifier: AGPL-3.0-or-later
use super::{CSS, DETAIL_JS, FORMAT_JS, INDEX, JS, RAW_JS, RENDER_JS, SESSIONS_JS, TOKENS};

#[test]
fn web_assets_do_not_seed_fixture_values() {
    let forbidden = [
        "web-smoke",
        "tool:bash",
        "web-rule",
        "web review",
        ">40<",
        "Capture pipeline",
        "Transcript tail -> events",
        "screen-heading",
        "screen-title",
        "screen-copy",
    ];
    for needle in forbidden {
        assert!(!INDEX.contains(needle), "index contains {needle}");
        assert!(!JS.contains(needle), "js contains {needle}");
    }
}

#[test]
fn web_assets_expose_read_only_visualization_screen() {
    let forbidden = [
        ">kaizen_",
        ">mcp/",
        "onclick=",
        "role=\"button\"",
        "data-tool",
        "data-feature",
        "href=\"/session-detail\"",
        "href=\"/experiments\"",
        "href=\"/settings\"",
    ];
    for needle in forbidden {
        assert!(!INDEX.contains(needle), "index contains {needle}");
    }
    assert!(INDEX.contains("<main"));
    assert!(INDEX.contains("observe-screen"));
    assert!(INDEX.contains("aria-live=\"polite\""));
    assert!(INDEX.contains("developer-raw"));
    assert!(INDEX.contains("Raw event payloads are omitted"));
    assert!(JS.contains("visualization_snapshot"));
    assert!(RENDER_JS.contains("renderReport"));
    assert!(CSS.contains(":focus-visible"));
}

#[test]
fn web_assets_define_truthful_observe_journey() {
    let ids = [
        "project-select",
        "manual-workspace",
        "journey-status",
        "detail-events",
        "detail-spans",
        "detail-files",
        "detail-tools",
        "detail-prompt",
        "project-insights",
        "insight-tools",
        "insight-attention",
        "insight-coverage",
        "developer-raw",
    ];
    for id in ids {
        assert!(
            INDEX.contains(&format!("id=\"{id}\"")),
            "index missing {id}"
        );
    }
    for needle in [
        "kaizen_sessions_list",
        "all_workspaces",
        "visibilitychange",
        "type: \"subscribe\"",
        "message.type === \"changed\"",
        "message.id !== state.snapshotPending",
        "Authorization required",
    ] {
        assert!(JS.contains(needle), "js missing {needle}");
    }
    assert!(!JS.contains("setInterval("));
    assert!(!INDEX.contains("See what your coding agents are doing."));
    assert!(!INDEX.contains("page-heading"));
    assert!(INDEX.contains("/assets/kaizen-tokens.css"));
    assert!(RENDER_JS.contains("renderInsights"));
    assert!(RENDER_JS.contains("topCommands(report?.selected?.events"));
    assert!(FORMAT_JS.contains("No completion event received"));
    assert!(DETAIL_JS.contains("event.payload?.summary"));
}

#[test]
fn web_assets_bound_large_report_rendering() {
    assert!(SESSIONS_JS.contains("MAX_SESSION_ROWS = 30"));
    assert!(SESSIONS_JS.contains("slice(0, MAX_SESSION_ROWS)"));
    assert!(!RENDER_JS.contains("JSON.stringify(report"));
    assert!(JS.contains("setRawReport(report)"));
    assert!(JS.contains("./kaizen-raw.js"));
    assert!(RAW_JS.contains("JSON.stringify(latestReport"));
    assert!(RAW_JS.contains("details.open"));
    assert!(SESSIONS_JS.contains("let renderedSessions = \"\""));
    assert!(SESSIONS_JS.contains("if (signature !== renderedSessions)"));
    assert!(SESSIONS_JS.contains("markSelected(selectedId)"));
}

#[test]
fn muted_text_meets_wcag_aa_on_paper() {
    assert!(TOKENS.contains("--ink-faint: #626b64;"));
    assert!(contrast([0x62, 0x6b, 0x64], [0xf4, 0xef, 0xe3]) >= 4.5);
    assert!(contrast([0x62, 0x6b, 0x64], [0xff, 0xfa, 0xf0]) >= 4.5);
}

fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let (bright, dark) = match luminance(a) > luminance(b) {
        true => (luminance(a), luminance(b)),
        false => (luminance(b), luminance(a)),
    };
    (bright + 0.05) / (dark + 0.05)
}

fn luminance(rgb: [u8; 3]) -> f64 {
    let [red, green, blue] = rgb.map(channel);
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

fn channel(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    match value <= 0.04045 {
        true => value / 12.92,
        false => ((value + 0.055) / 1.055).powf(2.4),
    }
}
