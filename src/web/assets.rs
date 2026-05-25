// SPDX-License-Identifier: AGPL-3.0-or-later
//! Embedded web assets.

pub const INDEX: &str = include_str!("assets/index.html");
pub const CSS: &str = include_str!("assets/kaizen.css");
pub const JS: &str = include_str!("assets/kaizen.js");

#[cfg(test)]
mod tests {
    use super::{INDEX, JS};

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
        ];
        for needle in forbidden {
            assert!(!INDEX.contains(needle), "index contains {needle}");
            assert!(!JS.contains(needle), "js contains {needle}");
        }
    }
}
