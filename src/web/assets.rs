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
    fn web_assets_expose_workflows_not_tool_labels() {
        for needle in [
            ">kaizen_",
            ">mcp/",
            "onclick=",
            "role=\"button\"",
            "data-tool",
        ] {
            assert!(!INDEX.contains(needle), "index contains {needle}");
        }
        for feature in crate::web::features::all() {
            assert!(
                INDEX.contains(&format!("data-feature=\"{}\"", feature.tool)),
                "missing visible workflow for {}",
                feature.tool
            );
        }
        assert!(INDEX.contains("developer-drawer"));
        assert!(INDEX.contains("data-feature=\"kaizen_summary\""));
        assert!(JS.contains("showModal"));
        assert!(RENDER_JS.contains("renderOutput"));
        assert!(CSS.contains(".drawer"));
    }
}
