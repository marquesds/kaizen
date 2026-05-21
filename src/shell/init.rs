// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen init` — idempotent workspace setup.

use crate::ipc::{CaptureComponentStatus, CaptureStatus};
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
const KAIZEN_EVAL_SKILL: &str = include_str!("../../assets/kaizen-eval-SKILL.md");

const CURSOR_HOOK_EVENTS: &[&str] = &["SessionStart", "PreToolUse", "PostToolUse", "Stop"];
const CLAUDE_HOOK_EVENTS: &[&str] = &["SessionStart", "PreToolUse", "PostToolUse", "Stop"];

#[derive(Clone, Copy, Debug, Default)]
pub struct InitOptions {
    pub deep: bool,
    pub start_capture: bool,
}

fn ts_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn backup_path(ws: &Path, filename: &str) -> Result<PathBuf> {
    let dir = crate::core::paths::project_data_dir(ws)?.join("backup");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.{}.bak", filename, ts_ms())))
}

fn ensure_config(out: &mut String, ws: &Path) -> Result<()> {
    let data_dir = crate::core::paths::project_data_dir(ws)?;
    let path = data_dir.join("config.toml");
    if path.exists() {
        writeln!(out, "  skipped  config.toml (project data dir)").unwrap();
        return Ok(());
    }
    std::fs::write(&path, CONFIG_TOML)?;
    writeln!(out, "  created  {}", path.display()).unwrap();
    Ok(())
}

/// Hook command string written to `.cursor/hooks.json`.
pub const KAIZEN_CURSOR_HOOK_CMD: &str = "kaizen ingest hook --source cursor";
pub const KAIZEN_OPENCLAW_HOOK_CMD: &str = "kaizen ingest hook --source openclaw";
const KAIZEN_OPENCLAW_SPAWN_ARGS: &str = r#""ingest", "hook", "--source", "openclaw""#;
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
    let Some(cursor_dir) = cursor_user_dir() else {
        writeln!(out, "  skipped  ~/.cursor/hooks.json (HOME unset)").unwrap();
        return Ok(());
    };
    let path = cursor_dir.join("hooks.json");
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut obj = serde_json::Map::new();
        let mut hooks = serde_json::Map::new();
        for event in CURSOR_HOOK_EVENTS {
            hooks.insert(
                (*event).to_string(),
                serde_json::json!([{"command": KAIZEN_CURSOR_HOOK_CMD}]),
            );
        }
        obj.insert("hooks".to_string(), serde_json::Value::Object(hooks));
        write_atomic(&path, &serde_json::to_string_pretty(&obj)?)?;
        writeln!(out, "  created  ~/.cursor/hooks.json").unwrap();
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut root: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            writeln!(out, "  error  ~/.cursor/hooks.json: {e}").unwrap();
            anyhow::bail!("malformed ~/.cursor/hooks.json: {e}");
        }
    };
    if cursor_hooks_done(&root) {
        writeln!(out, "  skipped  ~/.cursor/hooks.json").unwrap();
        return Ok(());
    }
    let bak = backup_path(ws, "cursor_hooks")?;
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
    write_atomic(&path, &serde_json::to_string_pretty(&root)?)?;
    writeln!(
        out,
        "  patched  ~/.cursor/hooks.json  (+session/tool hooks)"
    )
    .unwrap();
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
    let Some(claude_dir) = claude_user_dir() else {
        writeln!(out, "  skipped  ~/.claude/settings.json (HOME unset)").unwrap();
        return Ok(());
    };
    let path = claude_dir.join("settings.json");
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut obj = serde_json::Map::new();
        let mut hooks = serde_json::Map::new();
        for event in CLAUDE_HOOK_EVENTS {
            hooks.insert(
                (*event).to_string(),
                serde_json::json!([
                    {"hooks": [{"type": "command", "command": KAIZEN_CLAUDE_HOOK_CMD}]}
                ]),
            );
        }
        obj.insert("hooks".to_string(), serde_json::Value::Object(hooks));
        write_atomic(&path, &serde_json::to_string_pretty(&obj)?)?;
        writeln!(out, "  created  ~/.claude/settings.json").unwrap();
        return Ok(());
    }
    let raw = std::fs::read_to_string(&path)?;
    let mut obj: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            writeln!(out, "  error  ~/.claude/settings.json: {e}").unwrap();
            anyhow::bail!("malformed ~/.claude/settings.json: {e}");
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
            "  skipped  ~/.claude/settings.json  (already configured)"
        )
        .unwrap();
        return Ok(());
    }
    let bak = backup_path(ws, "claude_settings")?;
    std::fs::copy(&path, &bak)?;
    write_atomic(&path, &serde_json::to_string_pretty(&obj)?)?;
    writeln!(
        out,
        "  patched  ~/.claude/settings.json  (+session/tool hooks)"
    )
    .unwrap();
    Ok(())
}

/// Read-only: `~/.cursor/hooks.json` missing, valid JSON with full kaizen wiring, or not.
pub fn cursor_kaizen_hook_wiring(ws: &Path) -> Result<Option<bool>, String> {
    let _ = ws;
    let Some(cursor_dir) = cursor_user_dir() else {
        return Ok(None);
    };
    let path = cursor_dir.join("hooks.json");
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let root: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    Ok(Some(cursor_hooks_done(&root)))
}

/// Read-only: `~/.claude/settings.json` hooks all reference kaizen (same as post-patch `entry_has_kaizen_cmd`).
pub fn claude_kaizen_hook_wiring(ws: &Path) -> Result<Option<bool>, String> {
    let _ = ws;
    let Some(claude_dir) = claude_user_dir() else {
        return Ok(None);
    };
    let path = claude_dir.join("settings.json");
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

/// Returns any workspace-local files that still contain kaizen wiring (legacy from pre-global init).
pub fn detect_legacy_wiring(ws: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let cursor_local = ws.join(".cursor/hooks.json");
    if cursor_local.exists()
        && let Ok(raw) = std::fs::read_to_string(&cursor_local)
        && raw.contains(KAIZEN_CURSOR_HOOK_CMD)
    {
        found.push(cursor_local);
    }
    let claude_local = ws.join(".claude/settings.json");
    if claude_local.exists()
        && let Ok(raw) = std::fs::read_to_string(&claude_local)
        && raw.contains(KAIZEN_CLAUDE_HOOK_CMD)
    {
        found.push(claude_local);
    }
    found
}

fn write_eval_skill(out: &mut String, ws: &Path) -> Result<()> {
    let Some(cursor_dir) = cursor_user_dir() else {
        writeln!(
            out,
            "  skipped  ~/.cursor/skills/kaizen-eval/SKILL.md (HOME unset)"
        )
        .unwrap();
        return Ok(());
    };
    let path = cursor_dir.join("skills/kaizen-eval/SKILL.md");
    let _ = ws;
    std::fs::create_dir_all(path.parent().unwrap())?;
    if path.exists() {
        let existing = std::fs::read_to_string(&path)?;
        if !existing.contains("placeholder") && !existing.trim().is_empty() {
            writeln!(out, "  skipped  ~/.cursor/skills/kaizen-eval/SKILL.md").unwrap();
            return Ok(());
        }
    }
    std::fs::write(&path, KAIZEN_EVAL_SKILL)?;
    writeln!(out, "  wrote  ~/.cursor/skills/kaizen-eval/SKILL.md").unwrap();
    Ok(())
}

fn write_skill(out: &mut String, ws: &Path) -> Result<()> {
    let Some(cursor_dir) = cursor_user_dir() else {
        writeln!(
            out,
            "  skipped  ~/.cursor/skills/kaizen-retro/SKILL.md (HOME unset)"
        )
        .unwrap();
        return Ok(());
    };
    let path = cursor_dir.join("skills/kaizen-retro/SKILL.md");
    let _ = ws;
    std::fs::create_dir_all(path.parent().unwrap())?;
    if path.exists() {
        let existing = std::fs::read_to_string(&path)?;
        if !existing.contains("placeholder") && !existing.trim().is_empty() {
            writeln!(out, "  skipped  ~/.cursor/skills/kaizen-retro/SKILL.md").unwrap();
            return Ok(());
        }
    }
    std::fs::write(&path, KAIZEN_RETRO_SKILL)?;
    writeln!(out, "  wrote  ~/.cursor/skills/kaizen-retro/SKILL.md").unwrap();
    Ok(())
}

const OPENCLAW_HOOK_EVENTS: &[&str] = &[
    "message:received",
    "message:sent",
    "command:new",
    "command:reset",
    "command:stop",
    "session:compact:before",
    "session:compact:after",
    "session:patch",
];

const OPENCLAW_HANDLER_TS: &str = r#"import { spawn } from "child_process";

export async function handler(event: Record<string, unknown>) {
  const payload = JSON.stringify({
    event: event["type"] ?? event["event"],
    session_id: event["sessionId"] ?? event["session_id"] ?? "",
    timestamp_ms: typeof event["timestamp"] === "number" ? event["timestamp"] : Date.now(),
    ...event,
  });
  const child = spawn("kaizen", ["ingest", "hook", "--source", "openclaw"], {
    stdio: ["pipe", "ignore", "ignore"],
  });
  child.stdin?.write(payload + "\n");
  child.stdin?.end();
}
"#;

const OPENCLAW_HOOK_MD: &str = "# kaizen-events\n\nCaptures OpenClaw sessions for kaizen.\n";

fn cursor_user_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".cursor"))
}

fn claude_user_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".claude"))
}

fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    std::io::Write::write_all(&mut tmp, content.as_bytes())?;
    tmp.persist(path)?;
    Ok(())
}

fn openclaw_hooks_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".openclaw/hooks/kaizen-events"))
}

/// Write (or idempotently skip) the OpenClaw TS hook handler.
///
/// Backs up any pre-existing `handler.ts` that does not already reference kaizen.
pub fn patch_openclaw_handlers(out: &mut String, ws: &Path) -> Result<()> {
    let Some(hook_dir) = openclaw_hooks_dir() else {
        writeln!(
            out,
            "  skipped  ~/.openclaw/hooks/kaizen-events (HOME unset)"
        )
        .unwrap();
        return Ok(());
    };
    let handler_path = hook_dir.join("handler.ts");
    if handler_path.exists() {
        let existing = std::fs::read_to_string(&handler_path)?;
        if openclaw_handler_contains_kaizen(&existing) {
            writeln!(out, "  skipped  ~/.openclaw/hooks/kaizen-events/handler.ts").unwrap();
            return Ok(());
        }
        let bak = backup_path(ws, "openclaw_hook")?;
        std::fs::copy(&handler_path, &bak)?;
    }
    std::fs::create_dir_all(&hook_dir)?;
    std::fs::write(&handler_path, OPENCLAW_HANDLER_TS)?;
    std::fs::write(hook_dir.join("HOOK.md"), OPENCLAW_HOOK_MD)?;
    writeln!(out, "  created  ~/.openclaw/hooks/kaizen-events/handler.ts").unwrap();
    let _ = std::process::Command::new("openclaw")
        .args(["hooks", "enable", "kaizen-events"])
        .status();
    for event in OPENCLAW_HOOK_EVENTS {
        let _ = std::process::Command::new("openclaw")
            .args(["hooks", "subscribe", "kaizen-events", event])
            .status();
    }
    Ok(())
}

/// Read-only: `~/.openclaw/hooks/kaizen-events` absent / wired / partial.
pub fn openclaw_kaizen_hook_wiring(_ws: &Path) -> Result<Option<bool>, String> {
    let Some(hook_dir) = openclaw_hooks_dir() else {
        return Ok(None);
    };
    if !hook_dir.is_dir() {
        return Ok(None);
    }
    let handler_path = hook_dir.join("handler.ts");
    let hook_md = hook_dir.join("HOOK.md");
    if !handler_path.exists() || !hook_md.exists() {
        return Ok(Some(false));
    }
    let raw = std::fs::read_to_string(&handler_path).map_err(|e| e.to_string())?;
    Ok(Some(openclaw_handler_contains_kaizen(&raw)))
}

fn openclaw_handler_contains_kaizen(raw: &str) -> bool {
    raw.contains(KAIZEN_OPENCLAW_HOOK_CMD)
        || (raw.contains(r#"spawn("kaizen""#) && raw.contains(KAIZEN_OPENCLAW_SPAWN_ARGS))
}

/// Text that `kaizen init` would print to stdout.
pub fn init_text(workspace: Option<&std::path::Path>) -> Result<String> {
    init_text_with_options(workspace, InitOptions::default())
}

pub fn init_text_with_options(
    workspace: Option<&std::path::Path>,
    options: InitOptions,
) -> Result<String> {
    let ws = match workspace {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    let mut out = String::new();
    if let Ok(data_dir) = crate::core::paths::project_data_dir(&ws) {
        match crate::core::migrate_home::migrate_legacy_in_repo(&ws, &data_dir) {
            Ok(crate::core::migrate_home::MigrationOutcome::Migrated) => {
                writeln!(out, "  migrated  .kaizen/ → {}", data_dir.display()).unwrap();
            }
            Ok(crate::core::migrate_home::MigrationOutcome::Conflict) => {
                writeln!(
                    out,
                    "  warning  .kaizen/ and {} both non-empty — skipping auto-migration",
                    data_dir.display()
                )
                .unwrap();
            }
            _ => {}
        }
    }
    ensure_config(&mut out, &ws)?;
    patch_cursor_hooks(&mut out, &ws)?;
    patch_claude_settings(&mut out, &ws)?;
    patch_openclaw_handlers(&mut out, &ws)?;
    write_skill(&mut out, &ws)?;
    write_eval_skill(&mut out, &ws)?;
    let cws = crate::core::workspace::canonical(&ws);
    if let Err(e) = crate::core::machine_registry::record_init(&cws) {
        tracing::warn!("machine registry: {e:#}");
    }
    if options.start_capture {
        append_capture_status(&mut out, &cws, options.deep);
    }
    writeln!(out).unwrap();
    writeln!(
        out,
        "kaizen init complete — Cursor + Claude Code + OpenClaw hooks wired."
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "Run Cursor or Claude Code in this repo once, then:").unwrap();
    writeln!(
        out,
        "  kaizen summary            # cost + rollups (agent / model)"
    )
    .unwrap();
    writeln!(
        out,
        "  kaizen insights           # activity, top tools, guidance"
    )
    .unwrap();
    writeln!(out, "  kaizen tui                # live session browser").unwrap();
    writeln!(out, "  kaizen retro --days 7     # weekly heuristic bets").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "Agents: `kaizen mcp` exposes every command as MCP tools — see docs/mcp.md."
    )
    .unwrap();
    if let Ok(data_dir) = crate::core::paths::project_data_dir(&ws) {
        writeln!(out).unwrap();
        writeln!(out, "Project data: {}", data_dir.display()).unwrap();
    }
    Ok(out)
}

fn append_capture_status(out: &mut String, ws: &Path, deep: bool) {
    if !crate::daemon::enabled() {
        writeln!(out, "  skipped  daemon capture (KAIZEN_DAEMON=0)").unwrap();
        return;
    }
    let workspace = ws.to_string_lossy().to_string();
    match crate::daemon::ensure_capture_blocking(workspace, deep) {
        Ok(status) => write_capture_status(out, &status),
        Err(err) => writeln!(out, "  warning  daemon capture unavailable: {err:#}").unwrap(),
    }
}

fn write_capture_status(out: &mut String, status: &CaptureStatus) {
    writeln!(out, "  ready    daemon capture").unwrap();
    writeln!(
        out,
        "  ready    {}",
        status_line("watchers", &status.watchers)
    )
    .unwrap();
    writeln!(out, "  ready    {}", status_line("hooks", &status.hooks)).unwrap();
    if status.deep {
        writeln!(out, "  partial  deep capture ({})", status.proxies.len()).unwrap();
    }
    for err in &status.errors {
        writeln!(out, "  warning  {err}").unwrap();
    }
}

fn status_line(label: &str, components: &[crate::ipc::CaptureComponent]) -> String {
    let ready = components
        .iter()
        .filter(|c| c.status == CaptureComponentStatus::Ready)
        .count();
    format!("{label}: {ready}/{}", components.len())
}

/// Idempotent workspace setup.
pub fn cmd_init(workspace: Option<&Path>, deep: bool) -> Result<()> {
    print!(
        "{}",
        init_text_with_options(
            workspace,
            InitOptions {
                deep,
                start_capture: true,
            },
        )?
    );
    Ok(())
}
