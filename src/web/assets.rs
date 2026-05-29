// SPDX-License-Identifier: AGPL-3.0-or-later
//! Embedded web assets.

pub const INDEX: &str = include_str!("assets/index.html");
pub const CSS: &str = include_str!("assets/kaizen.css");
pub const JS: &str = include_str!("assets/kaizen.js");
pub const RENDER_JS: &str = include_str!("assets/kaizen-render.js");

#[cfg(test)]
mod tests {
    use super::{CSS, INDEX, JS, RENDER_JS};

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
            "session-detail",
            "experiments",
            "settings",
        ] {
            assert!(!INDEX.contains(needle), "index contains {needle}");
        }
        assert!(INDEX.contains("<main"));
        assert!(INDEX.contains("visualization-screen"));
        assert!(INDEX.contains("aria-live=\"polite\""));
        assert!(INDEX.contains("developer-drawer"));
        assert!(JS.contains("visualization_snapshot"));
        assert!(JS.contains("showModal"));
        assert!(RENDER_JS.contains("renderOutput"));
        assert!(CSS.contains(".drawer"));
    }
}
