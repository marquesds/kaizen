use super::*;
use kaizen::visualization::SessionSearchInput;

#[path = "search_edge.rs"]
mod edge;

#[test]
fn sessions_page_reports_filtered_total_and_next_offset() -> anyhow::Result<()> {
    let (_tmp, store) = seeded_store(31)?;
    let first = build_report(&store, search_query("", 0, 30))?;
    let last = build_report(&store, search_query("", 30, 30))?;
    assert_eq!(
        (first.sessions.len(), first.session_page.filtered_total),
        (30, 31)
    );
    assert_eq!(
        (first.session_page.offset, first.session_page.next_offset),
        (0, Some(30))
    );
    assert_eq!(
        (last.sessions.len(), last.session_page.next_offset),
        (1, None)
    );
    Ok(())
}

#[test]
fn prompt_match_ranks_before_newer_metadata_match() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    seed_named(
        &store,
        "prompt",
        1,
        "codex",
        "gpt",
        Some("needle in prompt"),
        None,
    )?;
    seed_named(&store, "metadata", 2, "needle-agent", "gpt", None, None)?;
    let report = build_report(&store, search_query("needle", 0, 30))?;
    assert_eq!(ids(&report), ["prompt", "metadata"]);
    assert_eq!(report.session_page.filtered_total, 2);
    Ok(())
}

#[test]
fn metadata_search_is_case_insensitive() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    seed_named(
        &store,
        "mixed",
        1,
        "Claude-Code",
        "Sonnet",
        None,
        Some("Feature/ABC"),
    )?;
    let report = build_report(&store, search_query("cLaUdE-cOdE", 0, 30))?;
    assert_eq!(ids(&report), ["mixed"]);
    Ok(())
}

#[test]
fn tool_name_matches_session() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    seed_named(&store, "tool", 1, "codex", "gpt", None, None)?;
    store.append_event(&tool_event("tool", "Read_File"))?;
    let report = build_report(&store, search_query("read_file", 0, 30))?;
    assert_eq!(ids(&report), ["tool"]);
    Ok(())
}

#[test]
fn reopen_backfills_prompt_projection_idempotently() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let path = tmp.path().join("k.db");
    let store = Store::open(&path)?;
    seed_named(
        &store,
        "old",
        1,
        "codex",
        "gpt",
        Some("historic prompt"),
        None,
    )?;
    drop(store);
    rusqlite::Connection::open(&path)?.execute("DELETE FROM session_search_prompts", [])?;
    let store = Store::open(&path)?;
    assert_eq!(
        ids(&build_report(&store, search_query("historic", 0, 30))?),
        ["old"]
    );
    drop(store);
    Store::open(&path)?;
    Ok(())
}

#[test]
fn pruning_session_removes_prompt_projection() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let path = tmp.path().join("k.db");
    let store = Store::open(&path)?;
    seed_named(&store, "old", 1, "codex", "gpt", Some("remove me"), None)?;
    store.prune_sessions_started_before(2)?;
    drop(store);
    let count: i64 = rusqlite::Connection::open(path)?.query_row(
        "SELECT COUNT(*) FROM session_search_prompts",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(count, 0);
    Ok(())
}

#[test]
fn empty_search_orders_newest_then_id() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    seed_named(&store, "b", 2, "codex", "gpt", None, None)?;
    seed_named(&store, "a", 2, "codex", "gpt", None, None)?;
    seed_named(&store, "old", 1, "codex", "gpt", None, None)?;
    assert_eq!(
        ids(&build_report(&store, search_query("", 0, 30))?),
        ["a", "b", "old"]
    );
    Ok(())
}

fn seeded_store(count: u64) -> anyhow::Result<(tempfile::TempDir, Store)> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    (0..count)
        .try_for_each(|n| seed_named(&store, &format!("s{n:02}"), n, "codex", "gpt", None, None))?;
    Ok((tmp, store))
}

fn seed_named(
    store: &Store,
    id: &str,
    at: u64,
    agent: &str,
    model: &str,
    prompt: Option<&str>,
    branch: Option<&str>,
) -> anyhow::Result<()> {
    let mut row = session(id, SessionStatus::Done);
    (row.started_at_ms, row.agent, row.model, row.branch) = (
        at,
        agent.into(),
        Some(model.into()),
        branch.map(str::to_string),
    );
    store.upsert_session(&row)?;
    prompt
        .map(|value| store.append_event(&prompt_event(id, value)))
        .transpose()?;
    Ok(())
}

fn search_query(q: &str, offset: usize, limit: usize) -> VisualizationQuery {
    let mut value = query(None);
    value.include_activity = false;
    value.limits.sessions = limit;
    value.session_search = SessionSearchInput {
        q: q.into(),
        offset,
    };
    value
}

fn ids(report: &kaizen::visualization::VisualizationReport) -> Vec<&str> {
    report.sessions.iter().map(|row| row.id.as_str()).collect()
}

fn prompt_event(id: &str, prompt: &str) -> Event {
    let mut value = event_at(id, 0, 1);
    value.kind = EventKind::Lifecycle;
    value.tool = None;
    value.payload = json!({"prompt": prompt});
    value
}

fn tool_event(id: &str, tool: &str) -> Event {
    let mut value = event_at(id, 0, 1);
    value.kind = EventKind::ToolCall;
    value.tool = Some(tool.into());
    value
}
