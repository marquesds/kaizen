// SPDX-License-Identifier: AGPL-3.0-or-later

#[test]
fn load_defaults_to_registered_workspaces() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(ws1)?;
    let ws2 = std::fs::canonicalize(ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    kaizen::core::workspace::resolve(Some(&ws1))?;
    kaizen::core::workspace::resolve(Some(&ws2))?;
    write_codex_session(home.path(), &ws1, "codex-load-1")?;
    write_claude_session(home.path(), &ws2, "claude-load-1")?;

    let text = load_text(None, true)?;
    let report: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(report["workspace_count"], 2);
    assert_eq!(report["totals"]["sessions_upserted"], 2);

    let ws1_sessions: serde_json::Value =
        serde_json::from_str(&sessions_list_text(Some(&ws1), true, false, false, None)?)?;
    let ws2_sessions: serde_json::Value =
        serde_json::from_str(&sessions_list_text(Some(&ws2), true, false, false, None)?)?;
    assert_eq!(ws1_sessions["sessions"][0]["agent"], "codex");
    assert_eq!(ws2_sessions["sessions"][0]["agent"], "claude");

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn load_workspace_limits_scope() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(ws1)?;
    let ws2 = std::fs::canonicalize(ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    write_codex_session(home.path(), &ws1, "codex-load-scope")?;
    write_claude_session(home.path(), &ws2, "claude-load-scope")?;

    let text = load_text(Some(&ws1), true)?;
    let report: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(report["workspace_count"], 1);
    assert_eq!(report["totals"]["sessions_upserted"], 1);
    let other: serde_json::Value =
        serde_json::from_str(&sessions_list_text(Some(&ws2), true, false, false, None)?)?;
    assert_eq!(other["count"], 0);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn cli_load_and_sessions_load_both_work() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    write_codex_session(home.path(), &ws, "codex-cli-load")?;

    let top = run_load_cmd(home.path(), &ws, &["load", "--workspace"])?;
    let nested = run_load_cmd(home.path(), &ws, &["sessions", "load", "--workspace"])?;
    assert_eq!(top["totals"]["sessions_upserted"], 1);
    assert_eq!(nested["totals"]["sessions_upserted"], 1);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

fn run_load_cmd(home: &Path, ws: &Path, prefix: &[&str]) -> anyhow::Result<serde_json::Value> {
    let mut args = prefix.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    args.push(ws.to_string_lossy().to_string());
    args.push("--json".into());
    let out = Command::new(env!("CARGO_BIN_EXE_kaizen"))
        .args(args)
        .env("HOME", home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(serde_json::from_slice(&out.stdout)?)
}
