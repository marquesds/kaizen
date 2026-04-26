// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nested `--help` on the real `kaizen` binary. When adding CLI surface, extend
//! `cli_help_matrix.inc` to match `src/main.rs` `Command` (and children).
//!
//! **Manual / TUI** (not automated): from `src/ui/tui.rs` and in-app `?` help,
//! exercise `j`/`k`, Tab, `m`, `/`, `y`, Enter, `g`/`G`, `r`, `?`, `q`/Esc,
//! plus resize; confirm session list and help render after a short run.

use std::process::Command;

const HELP_MATRIX: &[&[&str]] = include!("cli_help_matrix.inc");

fn kaizen_bin() -> &'static str {
    env!("CARGO_BIN_EXE_kaizen")
}

fn assert_help_ok(bin: &str, args: &[&str]) {
    let out = Command::new(bin)
        .args(args)
        .arg("--help")
        .output()
        .unwrap_or_else(|e| panic!("spawn kaizen: {e}"));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "args={args:?} stderr={stderr}");
    assert!(!out.stdout.is_empty(), "args={args:?}");
}

#[test]
fn nested_help_succeeds_on_binary() {
    let bin = kaizen_bin();
    for args in HELP_MATRIX {
        assert_help_ok(bin, args);
    }
}

#[test]
fn version_succeeds_on_binary() {
    let out = Command::new(kaizen_bin())
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("spawn: {e}"));
    assert!(out.status.success());
    let t = String::from_utf8_lossy(&out.stdout);
    assert!(t.contains("kaizen"), "{t}");
}
