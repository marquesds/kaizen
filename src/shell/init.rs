//! `kaizen init` — idempotent workspace setup.

use anyhow::Result;
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
const SKILL_PLACEHOLDER: &str = "# kaizen-retro skill placeholder\n";

const CURSOR_STOP_HOOK: &str =
    r#"{"matcher": "Stop", "command": "kaizen ingest hook --source cursor"}"#;
const CLAUDE_STOP_HOOK: &str =
    r#"{"type": "command", "command": "kaizen ingest hook --source claude"}"#;

fn ts_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn backup_path(ws: &Path, filename: &str) -> PathBuf {
    ws.join(format!(".kaizen/backup/{}.{}.bak", filename, ts_ms()))
}

fn ensure_config(ws: &Path) -> Result<()> {
    let path = ws.join(".kaizen/config.toml");
    if path.exists() {
        println!("  skipped  .kaizen/config.toml");
        return Ok(());
    }
    std::fs::create_dir_all(ws.join(".kaizen"))?;
    std::fs::write(&path, CONFIG_TOML)?;
    println!("  created  .kaizen/config.toml");
    Ok(())
}

const KAIZEN_HOOK_CMD: &str = "kaizen ingest hook --source cursor";

fn cursor_stop_already(root: &serde_json::Value) -> bool {
    // object format: {"hooks": {"Stop": [...]}}
    if let Some(arr) = root.pointer("/hooks/Stop").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .any(|v| v.get("command").and_then(|c| c.as_str()) == Some(KAIZEN_HOOK_CMD));
    }
    // legacy array format: [{matcher: "Stop", ...}]
    if let Some(arr) = root.as_array() {
        return arr
            .iter()
            .any(|v| v.get("matcher").and_then(|m| m.as_str()) == Some("Stop"));
    }
    false
}

fn patch_cursor_hooks(ws: &Path) -> Result<()> {
    let path = ws.join(".cursor/hooks.json");
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut root: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            println!("  error  .cursor/hooks.json: {e}");
            anyhow::bail!("malformed .cursor/hooks.json: {e}");
        }
    };
    if cursor_stop_already(&root) {
        println!("  skipped  .cursor/hooks.json");
        return Ok(());
    }
    let bak = backup_path(ws, "cursor_hooks");
    std::fs::create_dir_all(bak.parent().unwrap())?;
    std::fs::copy(&path, &bak)?;
    let entry = serde_json::json!({"command": KAIZEN_HOOK_CMD});
    if let Some(hooks) = root
        .pointer_mut("/hooks/Stop")
        .and_then(|v| v.as_array_mut())
    {
        hooks.push(entry);
    } else if let Some(obj) = root.pointer_mut("/hooks").and_then(|v| v.as_object_mut()) {
        obj.insert("Stop".into(), serde_json::json!([entry]));
    } else if let Some(arr) = root.as_array_mut() {
        arr.push(serde_json::from_str(CURSOR_STOP_HOOK)?);
    }
    std::fs::write(&path, serde_json::to_string_pretty(&root)?)?;
    println!("  patched  .cursor/hooks.json  (+Stop hook)");
    Ok(())
}

fn patch_claude_settings(ws: &Path) -> Result<()> {
    let path = ws.join(".claude/settings.json");
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut obj: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            println!("  error  .claude/settings.json: {e}");
            anyhow::bail!("malformed .claude/settings.json: {e}");
        }
    };
    let hooks = obj.entry("hooks").or_insert_with(|| serde_json::json!({}));
    let stop = hooks
        .as_object_mut()
        .unwrap()
        .entry("Stop")
        .or_insert_with(|| serde_json::json!([]));
    let already = stop.as_array().unwrap_or(&vec![]).iter().any(|v| {
        v.get("command").and_then(|c| c.as_str()) == Some("kaizen ingest hook --source claude")
    });
    if already {
        println!("  skipped  .claude/settings.json  (already configured)");
        return Ok(());
    }
    let bak = backup_path(ws, "claude_settings");
    std::fs::create_dir_all(bak.parent().unwrap())?;
    std::fs::copy(&path, &bak)?;
    stop.as_array_mut()
        .unwrap()
        .push(serde_json::from_str(CLAUDE_STOP_HOOK)?);
    std::fs::write(&path, serde_json::to_string_pretty(&obj)?)?;
    println!("  patched  .claude/settings.json  (+Stop hook)");
    Ok(())
}

fn write_skill(ws: &Path) -> Result<()> {
    let path = ws.join(".cursor/skills/kaizen-retro/SKILL.md");
    std::fs::create_dir_all(path.parent().unwrap())?;
    if !path.exists() {
        std::fs::write(&path, SKILL_PLACEHOLDER)?;
    }
    Ok(())
}

/// Idempotent workspace setup.
pub fn cmd_init(workspace: Option<&Path>) -> Result<()> {
    let ws = match workspace {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    ensure_config(&ws)?;
    patch_cursor_hooks(&ws)?;
    patch_claude_settings(&ws)?;
    write_skill(&ws)?;
    println!("\nkaizen init complete. Run: kaizen tui");
    Ok(())
}
