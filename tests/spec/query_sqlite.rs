// SPDX-License-Identifier: AGPL-3.0-or-later

use kaizen::core::event::{SessionRecord, SessionStatus};
use kaizen::store::{Store, query::QueryStore};

#[test]
fn summary_ignores_legacy_cold_files() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let workspace = dir.path().to_string_lossy().to_string();
    let store = Store::open(&dir.path().join("kaizen.db"))?;
    store.upsert_session(&session("s1", &workspace))?;
    let cold = dir.path().join("cold/events");
    std::fs::create_dir_all(&cold)?;
    std::fs::write(cold.join("legacy.parquet"), b"legacy")?;
    let query = QueryStore::open(dir.path())?;
    let stats = query.summary_stats(&store, &workspace)?;
    assert_eq!(stats.session_count, 1);
    assert_eq!(stats.total_cost_usd_e6, 0);
    assert_eq!(query.cold_event_count()?, 0);
    Ok(())
}

fn session(id: &str, workspace: &str) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt".into()),
        workspace: workspace.into(),
        started_at_ms: 1_700_000_000_000,
        ended_at_ms: Some(1_700_000_001_000),
        status: SessionStatus::Done,
        trace_path: String::new(),
        start_commit: None,
        end_commit: None,
        branch: None,
        dirty_start: None,
        dirty_end: None,
        repo_binding_source: None,
        prompt_fingerprint: None,
        parent_session_id: None,
        agent_version: None,
        os: None,
        arch: None,
        repo_file_count: None,
        repo_total_loc: None,
    }
}
