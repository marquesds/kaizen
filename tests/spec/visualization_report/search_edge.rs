use super::*;

const NOW_MS: u64 = 2_000_000;

#[test]
fn unicode_search_matches_metadata_and_prompt() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    seed_named(&store, "metadata", 2, "STRASSE", "gpt", None, None)?;
    seed_named(&store, "prompt", 1, "codex", "gpt", Some("STRASSE"), None)?;
    let report = build_report(&store, search_query("straße", 0, 30))?;
    assert_eq!(ids(&report), ["prompt", "metadata"]);
    Ok(())
}

#[test]
fn search_matches_displayed_derived_statuses_only() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    seed_statuses(&store)?;
    for (text, expected) in [("ctiv", "s1"), ("rror", "s2"), ("rphan", "s3")] {
        assert_eq!(ids(&build_report(&store, query_at(text))?), [expected]);
    }
    assert!(ids(&build_report(&store, query_at("running"))?).is_empty());
    Ok(())
}

#[test]
fn raw_idle_does_not_hide_displayed_error() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("k.db"))?;
    seed_raw_idle_error(&store)?;
    assert!(ids(&build_report(&store, query_at("idle"))?).is_empty());
    assert_eq!(ids(&build_report(&store, query_at("rror"))?), ["s6"]);
    Ok(())
}

fn seed_statuses(store: &Store) -> anyhow::Result<()> {
    seed_open(store, "s1", Some((NOW_MS - 1, EventKind::ToolCall)))?;
    seed_open(store, "s2", Some((NOW_MS - 1, EventKind::Error)))?;
    seed_open(store, "s3", Some((1, EventKind::ToolCall)))?;
    seed_open(store, "s4", None)?;
    seed_named(store, "s5", 1, "codex", "gpt", None, None)
}

fn seed_open(store: &Store, id: &str, observed: Option<(u64, EventKind)>) -> anyhow::Result<()> {
    let mut row = session(id, SessionStatus::Running);
    row.started_at_ms = 1;
    store.upsert_session(&row)?;
    observed
        .map(|(ts, kind)| store.append_event(&status_event(id, ts, kind)))
        .transpose()?;
    Ok(())
}

fn seed_raw_idle_error(store: &Store) -> anyhow::Result<()> {
    let mut row = session("s6", SessionStatus::Idle);
    row.started_at_ms = 1;
    store.upsert_session(&row)?;
    store.append_event(&status_event("s6", NOW_MS - 1, EventKind::Error))
}

fn status_event(id: &str, ts: u64, kind: EventKind) -> Event {
    let mut value = event_at(id, 0, ts);
    value.kind = kind;
    value
}

fn query_at(text: &str) -> VisualizationQuery {
    let mut value = search_query(text, 0, 30);
    value.now_ms = NOW_MS;
    value
}
