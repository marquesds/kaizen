// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen doctor` — workspace health (config, DB, hooks).

use crate::core::config;
use crate::shell::cli::{open_workspace_read_store, workspace_path};
use crate::shell::init;
use anyhow::Result;
use std::fmt::Write;
use std::io::IsTerminal;
use std::path::Path;

fn existing_ancestor(path: &Path) -> Option<&Path> {
    path.ancestors().find(|candidate| candidate.exists())
}

#[cfg(unix)]
fn writable_without_probe(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Some(path) = existing_ancestor(path) else {
        return false;
    };
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

#[cfg(not(unix))]
fn writable_without_probe(path: &Path) -> bool {
    existing_ancestor(path)
        .and_then(|candidate| candidate.metadata().ok())
        .is_some_and(|metadata| !metadata.permissions().readonly())
}

/// Runs checks; returns exit code (0 = ok, 1 = hard failure) and text for stdout.
pub fn doctor_text(workspace: Option<&Path>) -> Result<(i32, String)> {
    let ws = workspace_path(workspace)?;
    let mut hard_fail = false;
    let mut out = String::new();

    writeln!(&mut out, "kaizen {} (doctor)", env!("CARGO_PKG_VERSION")).unwrap();
    writeln!(&mut out, "workspace: {}", ws.display()).unwrap();
    writeln!(&mut out).unwrap();

    let data_dir = crate::core::paths::project_data_path(&ws).ok();
    let wcfg_ex = data_dir
        .as_ref()
        .is_some_and(|d| d.join("config.toml").exists());
    writeln!(
        &mut out,
        "project config.toml: {}",
        if wcfg_ex { "present" } else { "absent" }
    )
    .unwrap();
    match crate::core::paths::kaizen_dir() {
        Some(kd) => writeln!(
            &mut out,
            "~/.kaizen/config.toml: {}",
            if kd.join("config.toml").exists() {
                "present"
            } else {
                "absent"
            }
        )
        .unwrap(),
        None => writeln!(
            &mut out,
            "~/.kaizen/config.toml: (KAIZEN_HOME / HOME unset, skipped)"
        )
        .unwrap(),
    }
    match crate::core::machine_registry::status() {
        Ok(None) => writeln!(
            &mut out,
            "machine registry: (KAIZEN_HOME / HOME unset, skipped)"
        )
        .unwrap(),
        Ok(Some((ref path, n))) => writeln!(
            &mut out,
            "machine registry: OK ({}; {} project(s))",
            path.display(),
            n
        )
        .unwrap(),
        Err(e) => {
            hard_fail = true;
            writeln!(&mut out, "machine registry: ERROR: {e}").unwrap();
        }
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
        "  sources.tail: gemini={} pi={} kimi={} antigravity={} cursor_state_db={} goose={} opencode={} copilot_cli={} copilot_vscode={}",
        t.gemini, t.pi, t.kimi, t.antigravity, t.cursor_state_db, t.goose, t.opencode, t.copilot_cli, t.copilot_vscode
    )
    .unwrap();
    let sync_on = !cfg.sync.endpoint.is_empty() && !cfg.sync.team_id.is_empty();
    writeln!(&mut out, "  sync: endpoint configured: {}", sync_on).unwrap();
    writeln!(&mut out).unwrap();

    let db_result = crate::core::workspace::db_path(&ws);
    let ws_key = ws.to_string_lossy().to_string();
    match db_result.and_then(|db| open_workspace_read_store(&ws, false).map(|s| (db, s))) {
        Ok((db, store)) => {
            writeln!(&mut out, "store: OK ({})", db.display()).unwrap();
            if let Ok(sessions) = store.list_sessions(&ws_key) {
                writeln!(
                    &mut out,
                    "  sessions in store (this workspace key): {}",
                    sessions.len()
                )
                .unwrap();
            }
            if let Ok(data_dir) = crate::core::paths::project_data_path(&ws)
                && let Ok(query) = crate::store::query::QueryStore::open(&data_dir)
                && let Ok(stats) = query.summary_stats(&store, &ws_key)
                && crate::shell::cli::summary_needs_cost_rollup_note(
                    stats.session_count,
                    stats.total_cost_usd_e6,
                )
            {
                writeln!(
                    &mut out,
                    "  {}",
                    crate::shell::cli::cost_rollup_zero_doctor_hint()
                )
                .unwrap();
            }
            let probe = db.parent().map(|p| p.join(".kaizen_write_probe"));
            if let Some(probe) = probe {
                let ok = writable_without_probe(&probe);
                writeln!(
                    &mut out,
                    "project data dir writable: {}",
                    if ok { "yes" } else { "no" }
                )
                .unwrap();
                if !ok {
                    hard_fail = true;
                }
            }
        }
        Err(e) => {
            hard_fail = true;
            writeln!(&mut out, "store: ERROR: {e}").unwrap();
        }
    }
    writeln!(&mut out).unwrap();

    let cursor = init::cursor_kaizen_hook_wiring(&ws);
    match &cursor {
        Ok(None) => writeln!(
            &mut out,
            "hooks: ~/.cursor/hooks.json — absent (run `kaizen init` to wire Cursor)"
        )
        .unwrap(),
        Ok(Some(true)) => writeln!(
            &mut out,
            "hooks: ~/.cursor/hooks.json — kaizen command on all events"
        )
        .unwrap(),
        Ok(Some(false)) => {
            writeln!(&mut out, "hooks: ~/.cursor/hooks.json — present but not fully wired to kaizen (run: kaizen init)").unwrap();
        }
        Err(e) => writeln!(&mut out, "hooks: ~/.cursor/hooks.json — read error: {e}").unwrap(),
    }
    let claude = init::claude_kaizen_hook_wiring(&ws);
    match &claude {
        Ok(None) => writeln!(
            &mut out,
            "hooks: ~/.claude/settings.json — absent (run `kaizen init` to wire Claude Code)"
        )
        .unwrap(),
        Ok(Some(true)) => writeln!(
            &mut out,
            "hooks: ~/.claude/settings.json — kaizen hooks on all events"
        )
        .unwrap(),
        Ok(Some(false)) => {
            writeln!(
                &mut out,
                "hooks: ~/.claude/settings.json — present but not fully wired (run: kaizen init)"
            )
            .unwrap();
        }
        Err(e) => {
            writeln!(&mut out, "hooks: ~/.claude/settings.json — read error: {e}").unwrap();
        }
    }
    for path in init::detect_legacy_wiring(&ws) {
        writeln!(
            &mut out,
            "hooks: legacy local wiring at {} — safe to remove (kaizen now wires globally)",
            path.display()
        )
        .unwrap();
    }
    let openclaw = init::openclaw_kaizen_hook_wiring(&ws);
    match &openclaw {
        Ok(None) => writeln!(
            &mut out,
            "hooks: ~/.openclaw/hooks/kaizen-events — absent (run `kaizen init` to wire OpenClaw)"
        )
        .unwrap(),
        Ok(Some(true)) => {
            writeln!(&mut out, "hooks: ~/.openclaw/hooks/kaizen-events — wired").unwrap()
        }
        Ok(Some(false)) => writeln!(
            &mut out,
            "hooks: ~/.openclaw/hooks/kaizen-events — present but partial (run: kaizen init)"
        )
        .unwrap(),
        Err(e) => writeln!(
            &mut out,
            "hooks: ~/.openclaw/hooks/kaizen-events — read error: {e}"
        )
        .unwrap(),
    }
    writeln!(&mut out).unwrap();
    if std::io::stdout().is_terminal() {
        writeln!(&mut out, "If sessions list is empty, run a short agent session in this repo and `kaizen sessions list` again; see https://github.com/marquesds/kaizen/blob/main/docs/config.md#sources.").unwrap();
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
