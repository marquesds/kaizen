// SPDX-License-Identifier: AGPL-3.0-or-later

#[test]
fn summary_aggregates_registered_workspaces() -> anyhow::Result<()> {
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
    seed_session(&ws1, "s1", "read_file")?;
    seed_session(&ws2, "s2", "shell")?;

    let text = summary_text(Some(&ws1), true, false, true, DataSource::Local)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(json["session_count"], 2);
    assert_eq!(json["workspaces"].as_array().map(|v| v.len()), Some(2));

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn all_workspaces_includes_init_only_root_without_local_kaizen_db() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws1 = home.path().join("repo-a");
    let ws2 = home.path().join("repo-b");
    std::fs::create_dir_all(&ws1)?;
    std::fs::create_dir_all(&ws2)?;
    let ws1 = std::fs::canonicalize(&ws1)?;
    let ws2 = std::fs::canonicalize(&ws2)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    kaizen::core::workspace::resolve(Some(&ws1))?;
    kaizen::core::machine_registry::record_init(&ws2)?;
    assert!(
        !kaizen::core::workspace::db_path(&ws2).is_ok_and(|p| p.exists()),
        "test assumes second repo has no kaizen.db yet"
    );
    let roots = scope::resolve(Some(&ws1), true)?;
    assert_eq!(roots.len(), 2);
    assert!(roots.contains(&ws1));
    assert!(roots.contains(&ws2));
    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn sessions_list_stays_repo_scoped_without_machine_flag() -> anyhow::Result<()> {
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
    seed_session(&ws1, "s1", "read_file")?;
    seed_session(&ws2, "s2", "shell")?;

    let text = sessions_list_text(Some(&ws1), true, false, false, None)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(json["count"], 1);
    assert_eq!(json["sessions"][0]["id"], "s1");

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn default_reads_skip_global_scan_until_refresh() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    let cursor_slug = kaizen::core::paths::cursor_slug(&ws);
    let session = home
        .path()
        .join(".cursor/projects")
        .join(cursor_slug)
        .join("agent-transcripts/session-1");
    std::fs::create_dir_all(&session)?;
    std::fs::write(
        session.join("000.jsonl"),
        r#"{"message":{"content":[{"type":"tool_use","id":"toolu_1","name":"read_file","input":{"path":"src/main.rs"}}]}}"#,
    )?;

    let cold = sessions_list_text(Some(&ws), true, false, false, None)?;
    let cold_json: serde_json::Value = serde_json::from_str(&cold)?;
    assert_eq!(cold_json["count"], 0);
    let store = Store::open(&kaizen::core::workspace::db_path(&ws)?)?;
    assert_eq!(
        store.sync_state_get_u64(SYNC_STATE_LAST_AGENT_SCAN_MS)?,
        None
    );

    let refreshed = sessions_list_text(Some(&ws), true, true, false, None)?;
    let refreshed_json: serde_json::Value = serde_json::from_str(&refreshed)?;
    assert_eq!(refreshed_json["count"], 1);
    assert!(
        Store::open(&kaizen::core::workspace::db_path(&ws)?)?
            .sync_state_get_u64(SYNC_STATE_LAST_AGENT_SCAN_MS)?
            .is_some()
    );

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn sessions_list_refresh_sees_modern_agent_logs() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    write_codex_session(home.path(), &ws, "codex-refresh-1")?;
    write_claude_session(home.path(), &ws, "claude-refresh-1")?;
    write_gemini_session(&ws, "gemini-refresh-1")?;

    let text = sessions_list_text(Some(&ws), true, true, false, None)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(json["count"], 3);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}

#[test]
fn sessions_list_defaults_to_100_and_limit_zero_returns_all() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
    let home = TempDir::new()?;
    let ws = home.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    let ws = std::fs::canonicalize(ws)?;
    set_env("HOME", home.path());
    set_env("KAIZEN_HOME", home.path().join(".kaizen"));
    for i in 0..105 {
        seed_session(&ws, &format!("s{i:03}"), "read_file")?;
    }

    let capped = sessions_list_text(Some(&ws), true, false, false, None)?;
    let capped_json: serde_json::Value = serde_json::from_str(&capped)?;
    assert_eq!(capped_json["count"], 100);

    let custom = sessions_list_text(Some(&ws), true, false, false, Some(2))?;
    let custom_json: serde_json::Value = serde_json::from_str(&custom)?;
    assert_eq!(custom_json["count"], 2);

    let all = sessions_list_text(Some(&ws), true, false, false, Some(0))?;
    let all_json: serde_json::Value = serde_json::from_str(&all)?;
    assert_eq!(all_json["count"], 105);

    clear_env("KAIZEN_HOME");
    clear_env("HOME");
    Ok(())
}
