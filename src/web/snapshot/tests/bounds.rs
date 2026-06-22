use super::super::*;
use crate::core::event::SessionRecord;
use crate::store::Store;
use crate::visualization::BuiltReport;

const WORKSPACE: &str = "/bounded-web";

#[test]
fn web_materializes_only_bounded_latest_rows() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let store = Store::open(&temp.path().join("kaizen.db"))?;
    seed(&store)?;
    let built = build_snapshot(
        &store,
        WORKSPACE.into(),
        Some("s00".into()),
        Default::default(),
        100,
    )?;
    assert_materialized(&built);
    assert_selected(&built);
    Ok(())
}

#[test]
fn search_input_trims_without_echoing_query() -> anyhow::Result<()> {
    let input = search_input("  First Prompt  ", 30)?;
    assert_eq!(input.q, "First Prompt");
    assert_eq!(input.offset, 30);
    Ok(())
}

#[test]
fn search_input_rejects_more_than_256_characters() {
    let error = search_input(&"x".repeat(257), 0).unwrap_err().to_string();
    assert_eq!(error, "search query exceeds 256 characters");
}

#[test]
fn web_snapshot_applies_search_offset() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let store = Store::open(&temp.path().join("kaizen.db"))?;
    seed(&store)?;
    let search = SessionSearchInput {
        q: "S".into(),
        offset: 30,
    };
    let built = build_snapshot(&store, WORKSPACE.into(), None, search, 100)?;
    assert_eq!(built.report.sessions[0].id, "s00");
    assert_eq!(built.report.session_page.offset, 30);
    assert_eq!(built.report.session_page.next_offset, None);
    Ok(())
}

#[test]
fn read_only_web_snapshot_has_unicode_search() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("kaizen.db");
    let store = Store::open(&path)?;
    let mut record = session_at(1);
    record.agent = "AÇÃO".into();
    store.upsert_session(&record)?;
    drop(store);
    let store = Store::open_read_only(&path)?;
    let search = SessionSearchInput {
        q: "ação".into(),
        offset: 0,
    };
    let built = build_snapshot(&store, WORKSPACE.into(), None, search, 100)?;
    assert_eq!(built.report.sessions[0].id, "s01");
    Ok(())
}

fn seed(store: &Store) -> anyhow::Result<()> {
    (0..31).try_for_each(|index| store.upsert_session(&session_at(index)))?;
    (0..41).try_for_each(|index| seed_span(store, index))
}

fn seed_span(store: &Store, index: u64) -> anyhow::Result<()> {
    store.append_event(&event_at(
        index * 2,
        index,
        crate::core::event::EventKind::ToolCall,
    ))?;
    store.append_event(&event_at(
        index * 2 + 1,
        index,
        crate::core::event::EventKind::ToolResult,
    ))
}

fn session_at(index: u64) -> SessionRecord {
    let mut record = super::session(&format!("s{index:02}"));
    record.workspace = WORKSPACE.into();
    record.started_at_ms = index;
    record
}

fn event_at(
    seq: u64,
    index: u64,
    kind: crate::core::event::EventKind,
) -> crate::core::event::Event {
    let mut record = super::event(seq);
    record.session_id = "s00".into();
    record.kind = kind;
    record.tool = Some("read_file".into());
    record.tool_call_id = Some(format!("call-{index}"));
    record.payload = serde_json::json!({"input": {"path": format!("src/file-{index:02}.rs")}});
    record
}

fn assert_materialized(built: &BuiltReport) {
    assert_eq!(built.materialized.sessions, 30);
    assert_eq!(built.materialized.selected_events, 40);
    assert_eq!(built.materialized.selected_spans, 40);
    assert_eq!(built.materialized.selected_files, 40);
    assert_eq!(built.report.totals.session_count, 31);
    assert_eq!(built.report.totals.event_count, 82);
    assert_eq!(built.report.totals.tool_call_count, 41);
    assert_eq!(built.report.session_page.filtered_total, 31);
    assert_eq!(built.report.session_page.next_offset, Some(30));
}

fn assert_selected(built: &BuiltReport) {
    assert_eq!(built.report.sessions[0].id, "s30");
    let selected = built.report.selected.as_ref().unwrap();
    assert_eq!(selected.session.id, "s00");
    assert_eq!(selected.events.first().unwrap().seq, 42);
    assert_eq!(selected.events.last().unwrap().seq, 81);
    assert_eq!(selected.spans.len(), 40);
    assert_eq!(selected.files.len(), 40);
}
