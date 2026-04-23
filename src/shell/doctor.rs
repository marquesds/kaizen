// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen doctor` — workspace health (config, DB, hooks).

use crate::core::config;
use crate::shell::cli::workspace_path;
use crate::shell::init;
use crate::store::Store;
use anyhow::Result;
use std::fmt::Write;
use std::io::IsTerminal;
use std::path::Path;

/// Runs checks; returns exit code (0 = ok, 1 = hard failure) and text for stdout.
pub fn doctor_text(workspace: Option<&Path>) -> Result<(i32, String)> {
    let ws = workspace_path(workspace)?;
    let mut hard_fail = false;
    let mut out = String::new();

    writeln!(&mut out, "kaizen {} (doctor)", env!("CARGO_PKG_VERSION")).unwrap();
    writeln!(&mut out, "workspace: {}", ws.display()).unwrap();
    writeln!(&mut out).unwrap();

    let wcfg = ws.join(".kaizen/config.toml");
    let wcfg_ex = wcfg.exists();
    writeln!(
        &mut out,
        ".kaizen/config.toml: {}",
        if wcfg_ex { "present" } else { "absent" }
    )
    .unwrap();
    if let Ok(home) = std::env::var("HOME") {
        let p = Path::new(&home).join(".kaizen/config.toml");
        writeln!(
            &mut out,
            "~/.kaizen/config.toml: {}",
            if p.exists() { "present" } else { "absent" }
        )
        .unwrap();
    } else {
        writeln!(&mut out, "~/.kaizen/config.toml: (HOME unset, skipped)").unwrap();
    }
    writeln!(&mut out).unwrap();

    let cfg = match config::load(&ws) {
        Ok(c) => c,
        Err(e) => {
            writeln!(&mut out, "config load: ERROR: {e}").unwrap();
            return Ok((1, out));
        }
    };

    writeln!(&mut out, "config (merged, no secrets):").unwrap();
    writeln!(&mut out, "  scan.roots: {} entries", cfg.scan.roots.len()).unwrap();
    for (i, r) in cfg.scan.roots.iter().take(3).enumerate() {
        let exp = crate::shell::cli::expand_home(r);
        let exists = Path::new(&exp).exists();
        writeln!(&mut out, "    [{}] {} → exists={}", i + 1, r, exists).unwrap();
    }
    if cfg.scan.roots.len() > 3 {
        writeln!(&mut out, "    …").unwrap();
    }
    writeln!(
        &mut out,
        "  sources.cursor: enabled={} glob={}",
        cfg.sources.cursor.enabled, cfg.sources.cursor.transcript_glob
    )
    .unwrap();
    let t = &cfg.sources.tail;
    writeln!(
        &mut out,
        "  sources.tail: goose={} opencode={} copilot_cli={} copilot_vscode={}",
        t.goose, t.opencode, t.copilot_cli, t.copilot_vscode
    )
    .unwrap();
    let sync_on = !cfg.sync.endpoint.is_empty() && !cfg.sync.team_id.is_empty();
    writeln!(&mut out, "  sync: endpoint configured: {}", sync_on).unwrap();
    writeln!(&mut out).unwrap();

    let db = ws.join(".kaizen/kaizen.db");
    let ws_key = ws.to_string_lossy().to_string();
    match Store::open(&db) {
        Ok(store) => {
            writeln!(&mut out, "store: OK ({})", db.display()).unwrap();
            if let Ok(sessions) = store.list_sessions(&ws_key) {
                writeln!(
                    &mut out,
                    "  sessions in store (this workspace key): {}",
                    sessions.len()
                )
                .unwrap();
            }
        }
        Err(e) => {
            hard_fail = true;
            writeln!(&mut out, "store: ERROR: {e}").unwrap();
        }
    }
    if let Some(parent) = db.parent() {
        if !parent.exists() {
            writeln!(
                &mut out,
                ".kaizen/ directory: missing (will be created on first open)"
            )
            .unwrap();
        } else {
            let probe = parent.join(".kaizen_write_probe");
            let ok = std::fs::File::create(&probe).is_ok();
            if ok {
                let _ = std::fs::remove_file(&probe);
            }
            writeln!(
                &mut out,
                ".kaizen/ writable: {}",
                if ok { "yes" } else { "no" }
            )
            .unwrap();
            if !ok {
                hard_fail = true;
            }
        }
    }
    writeln!(&mut out).unwrap();

    let cursor = init::cursor_kaizen_hook_wiring(&ws);
    match &cursor {
        Ok(None) => writeln!(&mut out, "hooks: .cursor/hooks.json — absent (optional)").unwrap(),
        Ok(Some(true)) => writeln!(
            &mut out,
            "hooks: .cursor/hooks.json — kaizen command on all events"
        )
        .unwrap(),
        Ok(Some(false)) => {
            writeln!(&mut out, "hooks: .cursor/hooks.json — present but not fully wired to kaizen (run: kaizen init)").unwrap();
        }
        Err(e) => writeln!(&mut out, "hooks: .cursor/hooks.json — read error: {e}").unwrap(),
    }
    let claude = init::claude_kaizen_hook_wiring(&ws);
    match &claude {
        Ok(None) => writeln!(&mut out, "hooks: .claude/settings.json — absent (optional)").unwrap(),
        Ok(Some(true)) => writeln!(
            &mut out,
            "hooks: .claude/settings.json — kaizen hooks on all events"
        )
        .unwrap(),
        Ok(Some(false)) => {
            writeln!(
                &mut out,
                "hooks: .claude/settings.json — present but not fully wired (run: kaizen init)"
            )
            .unwrap();
        }
        Err(e) => writeln!(&mut out, "hooks: .claude/settings.json — read error: {e}").unwrap(),
    }
    writeln!(&mut out).unwrap();
    if std::io::stdout().is_terminal() {
        writeln!(&mut out, "If sessions list is empty, run a short agent session in this repo and `kaizen sessions list` again; see https://github.com/lucasmarqs/kaizen/blob/main/docs/config.md#sources.").unwrap();
    } else {
        writeln!(&mut out, "If sessions list is empty, see docs/config.md (sources) and `kaizen doctor` from a TTY for tips.").unwrap();
    }
    if hard_fail {
        Ok((1, out))
    } else {
        Ok((0, out))
    }
}

pub fn cmd_doctor(workspace: Option<&Path>) -> Result<i32> {
    let (code, s) = doctor_text(workspace)?;
    print!("{s}");
    Ok(code)
}
