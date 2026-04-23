// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen init` — idempotent workspace setup.

use anyhow::Result;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CONFIG_TOML: &str = r#"[kaizen]

# Optional sync (usually override secrets in ~/.kaizen/config.toml):
# [sync]
# endpoint = "https://ingest.example.com"
# team_token = "Bearer-token-from-server"
# team_id = "your-team"
# events_per_batch_max = 500
# max_body_bytes = 1000000
# flush_interval_ms = 10000
# sample_rate = 1.0
"#;
const KAIZEN_RETRO_SKILL: &str = include_str!("../../assets/kaizen-retro-SKILL.md");

const CURSOR_HOOK_EVENTS: &[&str] = &["SessionStart", "PreToolUse", "PostToolUse", "Stop"];
const CLAUDE_HOOK_EVENTS: &[&str] = &["SessionStart", "PreToolUse", "PostToolUse", "Stop"];

fn ts_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn backup_path(ws: &Path, filename: &str) -> PathBuf {
    ws.join(format!(".kaizen/backup/{}.{}.bak", filename, ts_ms()))
}

fn ensure_config(out: &mut String, ws: &Path) -> Result<()> {
    let path = ws.join(".kaizen/config.toml");
    if path.exists() {
        writeln!(out, "  skipped  .kaizen/config.toml").unwrap();
        return Ok(());
    }
    std::fs::create_dir_all(ws.join(".kaizen"))?;
    std::fs::write(&path, CONFIG_TOML)?;
    writeln!(out, "  created  .kaizen/config.toml").unwrap();
    Ok(())
}

/// Hook command string written to `.cursor/hooks.json`.
pub const KAIZEN_CURSOR_HOOK_CMD: &str = "kaizen ingest hook --source cursor";
/// Hook command string written to `.claude/settings.json`.
pub const KAIZEN_CLAUDE_HOOK_CMD: &str = "kaizen ingest hook --source claude";

/// `true` if every Cursor hook event points at the kaizen ingest command.
fn cursor_hooks_done(root: &serde_json::Value) -> bool {
    CURSOR_HOOK_EVENTS
        .iter()
        .all(|event| cursor_hook_exists(root, event))
}

fn cursor_hook_exists(root: &serde_json::Value, event: &str) -> bool {
    if let Some(arr) = root
        .pointer(&format!("/hooks/{event}"))
        .and_then(|v| v.as_array())
    {
        return arr
            .iter()
            .any(|v| v.get("command").and_then(|c| c.as_str()) == Some(KAIZEN_CURSOR_HOOK_CMD));
    }
    if let Some(arr) = root.as_array() {
        return arr.iter().any(|v| {
            v.get("matcher").and_then(|m| m.as_str()) == Some(event)
                && v.get("command").and_then(|c| c.as_str()) == Some(KAIZEN_CURSOR_HOOK_CMD)
        });
    }
    false
}

fn patch_cursor_hooks(out: &mut String, ws: &Path) -> Result<()> {
    let path = ws.join(".cursor/hooks.json");
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut root: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            writeln!(out, "  error  .cursor/hooks.json: {e}").unwrap();
            anyhow::bail!("malformed .cursor/hooks.json: {e}");
        }
    };
    if cursor_hooks_done(&root) {
        writeln!(out, "  skipped  .cursor/hooks.json").unwrap();
        return Ok(());
    }
    let bak = backup_path(ws, "cursor_hooks");
    std::fs::create_dir_all(bak.parent().unwrap())?;
    std::fs::copy(&path, &bak)?;
    if let Some(obj) = root.pointer_mut("/hooks").and_then(|v| v.as_object_mut()) {
        for event in CURSOR_HOOK_EVENTS {
            let arr = obj
                .entry((*event).to_string())
                .or_insert_with(|| serde_json::json!([]));
            if let Some(hooks) = arr.as_array_mut()
                && !hooks.iter().any(|v| {
                    v.get("command").and_then(|c| c.as_str()) == Some(KAIZEN_CURSOR_HOOK_CMD)
                })
            {
                hooks.push(serde_json::json!({"command": KAIZEN_CURSOR_HOOK_CMD}));
            }
        }
    } else if let Some(arr) = root.as_array_mut() {
        for event in CURSOR_HOOK_EVENTS {
            if !cursor_hook_exists(&serde_json::Value::Array(arr.clone()), event) {
                arr.push(serde_json::json!({"matcher": event, "command": KAIZEN_CURSOR_HOOK_CMD}));
            }
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(&root)?)?;
    writeln!(out, "  patched  .cursor/hooks.json  (+session/tool hooks)").unwrap();
    Ok(())
}

fn entry_has_kaizen_cmd(entry: &serde_json::Value) -> bool {
    if entry.get("command").and_then(|c| c.as_str()) == Some(KAIZEN_CLAUDE_HOOK_CMD) {
        return true;
    }
    entry
        .get("hooks")
        .and_then(|v| v.as_array())
        .is_some_and(|inner| {
            inner
                .iter()
                .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(KAIZEN_CLAUDE_HOOK_CMD))
        })
}

fn patch_claude_settings(out: &mut String, ws: &Path) -> Result<()> {
    let path = ws.join(".claude/settings.json");
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut obj: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            writeln!(out, "  error  .claude/settings.json: {e}").unwrap();
            anyhow::bail!("malformed .claude/settings.json: {e}");
        }
    };
    let hooks = obj.entry("hooks").or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks.as_object_mut().unwrap();
    let mut changed = false;
    for event in CLAUDE_HOOK_EVENTS {
        let arr = hooks_obj
            .entry((*event).to_string())
            .or_insert_with(|| serde_json::json!([]));
        let Some(entries) = arr.as_array_mut() else {
            continue;
        };
        // Migrate any bare {command,type} entries missing the `hooks` wrapper.
        for entry in entries.iter_mut() {
            if entry.get("hooks").is_some() {
                continue;
            }
            if let Some(obj) = entry.as_object()
                && obj.contains_key("command")
            {
                let inner = entry.clone();
                *entry = serde_json::json!({ "hooks": [inner] });
                changed = true;
            }
        }
        if !entries.iter().any(entry_has_kaizen_cmd) {
            entries.push(serde_json::json!({
                "hooks": [
                    {"type": "command", "command": KAIZEN_CLAUDE_HOOK_CMD}
                ]
            }));
            changed = true;
        }
    }
    if !changed {
        writeln!(
            out,
            "  skipped  .claude/settings.json  (already configured)"
        )
        .unwrap();
        return Ok(());
    }
    let bak = backup_path(ws, "claude_settings");
    std::fs::create_dir_all(bak.parent().unwrap())?;
    std::fs::copy(&path, &bak)?;
    std::fs::write(&path, serde_json::to_string_pretty(&obj)?)?;
    writeln!(
        out,
        "  patched  .claude/settings.json  (+session/tool hooks)"
    )
    .unwrap();
    Ok(())
}

/// Read-only: `.cursor/hooks.json` missing, valid JSON with full kaizen wiring, or not.
pub fn cursor_kaizen_hook_wiring(ws: &Path) -> Result<Option<bool>, String> {
    let path = ws.join(".cursor/hooks.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(Some(cursor_hooks_done(&root)))
}

/// Read-only: `.claude/settings.json` hooks all reference kaizen (same as post-patch `entry_has_kaizen_cmd`).
pub fn claude_kaizen_hook_wiring(ws: &Path) -> Result<Option<bool>, String> {
    let path = ws.join(".claude/settings.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let obj: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let Some(hooks) = obj.get("hooks").and_then(|v| v.as_object()) else {
        return Ok(Some(false));
    };
    for event in CLAUDE_HOOK_EVENTS {
        let Some(arr) = hooks.get(*event).and_then(|v| v.as_array()) else {
            return Ok(Some(false));
        };
        if !arr.iter().any(entry_has_kaizen_cmd) {
            return Ok(Some(false));
        }
    }
    Ok(Some(true))
}

fn write_skill(out: &mut String, ws: &Path) -> Result<()> {
    let path = ws.join(".cursor/skills/kaizen-retro/SKILL.md");
    std::fs::create_dir_all(path.parent().unwrap())?;
    if path.exists() {
        let existing = std::fs::read_to_string(&path)?;
        if !existing.contains("placeholder") && !existing.trim().is_empty() {
            writeln!(out, "  skipped  .cursor/skills/kaizen-retro/SKILL.md").unwrap();
            return Ok(());
        }
    }
    std::fs::write(&path, KAIZEN_RETRO_SKILL)?;
    writeln!(out, "  wrote  .cursor/skills/kaizen-retro/SKILL.md").unwrap();
    Ok(())
}

/// Text that `kaizen init` would print to stdout.
pub fn init_text(workspace: Option<&std::path::Path>) -> Result<String> {
    let ws = match workspace {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let mut out = String::new();
    ensure_config(&mut out, &ws)?;
    patch_cursor_hooks(&mut out, &ws)?;
    patch_claude_settings(&mut out, &ws)?;
    write_skill(&mut out, &ws)?;
    writeln!(out).unwrap();
    writeln!(out, "kaizen init complete.").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Next steps:").unwrap();
    writeln!(
        out,
        "  1. kaizen sessions list   # see indexed sessions (run an agent in this repo if empty)"
    )
    .unwrap();
    writeln!(
        out,
        "  2. kaizen summary         # cost and rollups by agent / model"
    )
    .unwrap();
    writeln!(
        out,
        "  3. kaizen metrics         # or: kaizen tui  (browse + live tail)"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "MCP: run `kaizen mcp` (stdio) — https://github.com/lucasmarqs/kaizen/blob/main/docs/mcp.md"
    )
    .unwrap();
    Ok(out)
}

/// Idempotent workspace setup.
pub fn cmd_init(workspace: Option<&Path>) -> Result<()> {
    print!("{}", init_text(workspace)?);
    Ok(())
}
