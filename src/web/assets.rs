// SPDX-License-Identifier: AGPL-3.0-or-later
//! Embedded web assets.

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;

pub const INDEX: &str = include_str!("assets/index.html");
pub const TOKENS: &str = include_str!("assets/kaizen-tokens.css");
pub const CSS: &str = include_str!("assets/kaizen.css");
pub const JS: &str = include_str!("assets/kaizen.js");
pub const STATE_JS: &str = include_str!("assets/kaizen-state.js");
pub const TRANSPORT_JS: &str = include_str!("assets/kaizen-transport.js");
pub const RENDER_JS: &str = include_str!("assets/kaizen-render.js");
pub const RAW_JS: &str = include_str!("assets/kaizen-raw.js");
pub const DETAIL_JS: &str = include_str!("assets/kaizen-detail.js");
pub const FORMAT_JS: &str = include_str!("assets/kaizen-format.js");

pub async fn index() -> impl IntoResponse {
    content("text/html; charset=utf-8", INDEX)
}

pub async fn tokens() -> impl IntoResponse {
    content("text/css; charset=utf-8", TOKENS)
}

pub async fn css() -> impl IntoResponse {
    content("text/css; charset=utf-8", CSS)
}

pub async fn js() -> impl IntoResponse {
    content("application/javascript", JS)
}

pub async fn state_js() -> impl IntoResponse {
    content("application/javascript", STATE_JS)
}

pub async fn transport_js() -> impl IntoResponse {
    content("application/javascript", TRANSPORT_JS)
}

pub async fn render_js() -> impl IntoResponse {
    content("application/javascript", RENDER_JS)
}

pub async fn raw_js() -> impl IntoResponse {
    content("application/javascript", RAW_JS)
}

pub async fn detail_js() -> impl IntoResponse {
    content("application/javascript", DETAIL_JS)
}

pub async fn format_js() -> impl IntoResponse {
    content("application/javascript", FORMAT_JS)
}

fn content(kind: &'static str, body: &'static str) -> impl IntoResponse {
    ([(CONTENT_TYPE, kind), (CACHE_CONTROL, "no-store")], body)
}

#[cfg(test)]
mod tests {
    use super::{CSS, INDEX, JS, RAW_JS, RENDER_JS, TOKENS};

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
        for needle in [
            ">kaizen_",
            ">mcp/",
            "onclick=",
            "role=\"button\"",
            "data-tool",
            "data-feature",
            "href=\"/session-detail\"",
            "href=\"/experiments\"",
            "href=\"/settings\"",
        ] {
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
        for needle in [
            "id=\"project-select\"",
            "id=\"manual-workspace\"",
            "id=\"journey-status\"",
            "id=\"detail-events\"",
            "id=\"detail-spans\"",
            "id=\"detail-files\"",
            "id=\"detail-tools\"",
            "id=\"developer-raw\"",
            "/assets/kaizen-tokens.css",
        ] {
            assert!(INDEX.contains(needle), "index missing {needle}");
        }
        for needle in [
            "kaizen_sessions_list",
            "all_workspaces",
            "visibilitychange",
            "AUTO_REFRESH_MS",
            "Authorization required",
        ] {
            assert!(JS.contains(needle), "js missing {needle}");
        }
        assert!(!JS.contains("setInterval("));
    }

    #[test]
    fn web_assets_bound_large_report_rendering() {
        assert!(RENDER_JS.contains("MAX_SESSION_ROWS = 30"));
        assert!(RENDER_JS.contains("sessions.slice(0, MAX_SESSION_ROWS)"));
        assert!(RENDER_JS.contains("of ${count(total)} newest"));
        assert!(!RENDER_JS.contains("JSON.stringify(report"));
        assert!(JS.contains("setRawReport(report)"));
        assert!(JS.contains("./kaizen-raw.js"));
        assert!(RAW_JS.contains("JSON.stringify(latestReport"));
        assert!(RAW_JS.contains("details.open"));
        assert!(RENDER_JS.contains("let renderedSessions = \"\""));
        assert!(RENDER_JS.contains("if (signature !== renderedSessions)"));
        assert!(RENDER_JS.contains("markSelected(selectedId)"));
        assert!(RENDER_JS.contains("report?.totals?.session_count"));
        assert!(JS.contains("visible === total"));
    }

    #[test]
    fn muted_text_meets_wcag_aa_on_paper() {
        let muted = [0x62, 0x6b, 0x64];
        assert!(TOKENS.contains("--ink-faint: #626b64;"));
        assert!(contrast(muted, [0xf4, 0xef, 0xe3]) >= 4.5);
        assert!(contrast(muted, [0xff, 0xfa, 0xf0]) >= 4.5);
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
}
