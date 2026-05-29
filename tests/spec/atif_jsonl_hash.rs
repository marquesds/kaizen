// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::extensions::{aggregates, atif, diffs, hash_chain, jsonl};
use kaizen::interchange::atif as pure_atif;
use kaizen::interchange::diff_attribution::{DiffAttribution, DiffAttributionOptions};
use kaizen::interchange::hash_chain::{HashChainEvent, compute_hash_chain, verify_hash_chain};
use kaizen::interchange::jsonl::parse_jsonl_events;
use kaizen::store::Store;
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn aggregate_hash_and_atif_round_trip() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path().join("repo");
    let db = tmp.path().join("kaizen.db");
    let store = Store::open(&db)?;
    store.upsert_session(&session("s1", workspace.to_string_lossy().as_ref()))?;
    store.append_event(&event("s1", 0))?;
    store.append_event(&event("s1", 1))?;

    let agg = aggregates::get(&store, "s1")?.expect("aggregate");
    assert_eq!(agg.event_count, 2);
    assert_eq!(
        hash_chain::verify(&store, workspace.to_string_lossy().as_ref(), Some("s1"))?
            .verified_events,
        2
    );

    let mut doc = atif::export_session(&store, "s1")?;
    drop(store);
    let imported = Store::open(&tmp.path().join("imported.db"))?;
    atif::import_document(&imported, &mut doc, "/imported")?;
    assert_eq!(imported.list_events_for_session("s1")?.len(), 2);
    assert_eq!(imported.get_session("s1")?.unwrap().workspace, "/imported");
    Ok(())
}

#[test]
fn jsonl_import_accepts_generic_fixture() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("kaizen.db"))?;

    let report = jsonl::import_file(
        &store,
        std::path::Path::new("tests/fixtures/interchange/generic_events.jsonl"),
        "/workspace",
    )?;
    assert_eq!(report.imported_events, 2);
    assert_eq!(report.sessions_created, 1);
    assert_eq!(store.list_events_for_session("sess-jsonl")?.len(), 2);
    Ok(())
}

#[test]
fn missing_hash_rows_are_legacy_unverifiable() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let db = tmp.path().join("kaizen.db");
    let store = Store::open(&db)?;
    store.upsert_session(&session("legacy", "/workspace"))?;
    store.append_event(&event("legacy", 0))?;
    drop(store);

    rusqlite::Connection::open(&db)?.execute("DELETE FROM event_hashes", [])?;
    let store = Store::open(&db)?;
    let report = hash_chain::verify(&store, "/workspace", Some("legacy"))?;
    assert_eq!(report.unverifiable_events, 1);
    assert!(report.broken_events.is_empty());
    Ok(())
}

#[test]
fn diff_attribution_omits_raw_patch_by_default() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let store = Store::open(&tmp.path().join("kaizen.db"))?;
    store.upsert_session(&session("diffs", "/workspace"))?;

    let rows = diffs::refresh_session(&store, "diffs", false)?;
    assert!(rows.iter().all(|row| !row.raw_patch_stored));
    Ok(())
}

#[test]
fn pure_atif_jsonl_and_hash_chain_round_trip() {
    let raw = include_str!("../fixtures/interchange/generic_events.jsonl");
    let events = parse_jsonl_events(raw).unwrap();
    let trace = pure_atif::trace_from_jsonl(pure_session("sess-jsonl"), events);
    let doc = pure_atif::export_atif(&trace);
    let chain = compute_hash_chain(&hash_inputs(&doc.events));

    assert_eq!(pure_atif::import_atif(&doc).unwrap(), trace);
    assert!(verify_hash_chain(&hash_inputs(&doc.events), &chain).is_ok());
}

#[test]
fn pure_jsonl_error_reports_source_line() {
    let err = parse_jsonl_events(
        "{\"session_id\":\"s\",\"seq\":0,\"ts_ms\":1,\"kind\":\"message\"}\n{bad",
    )
    .unwrap_err();

    assert_eq!(err.line, 2);
}

#[test]
fn pure_diff_attribution_omits_raw_patch_by_default() {
    let off = DiffAttribution::new("sess-diff", "abc").with_raw_patch("secret", Default::default());
    let on = DiffAttribution::new("sess-diff", "abc").with_raw_patch(
        "diff --git a/x b/x",
        DiffAttributionOptions::include_raw_patch(),
    );

    assert_eq!(json!(off).get("raw_patch"), None);
    assert_eq!(json!(on)["raw_patch"], "diff --git a/x b/x");
}

fn session(id: &str, workspace: &str) -> SessionRecord {
    SessionRecord {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt-5".into()),
        workspace: workspace.into(),
        started_at_ms: 1_000,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: "/trace".into(),
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

fn event(session_id: &str, seq: u64) -> Event {
    Event {
        session_id: session_id.into(),
        seq,
        ts_ms: 1_000 + seq,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Tail,
        tool: Some("bash".into()),
        tool_call_id: None,
        tokens_in: Some(10),
        tokens_out: Some(20),
        reasoning_tokens: Some(3),
        cost_usd_e6: Some(42),
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: Some(4),
        cache_read_tokens: Some(5),
        system_prompt_tokens: None,
        payload: json!({"seq": seq}),
    }
}

fn pure_session(id: &str) -> pure_atif::InterchangeSession {
    pure_atif::InterchangeSession {
        id: id.into(),
        agent: "codex".into(),
        model: Some("gpt-5".into()),
        workspace: Some("/repo".into()),
        started_at_ms: 1,
        ended_at_ms: None,
        attributes: BTreeMap::new(),
    }
}

fn hash_inputs(events: &[pure_atif::AtifEvent]) -> Vec<HashChainEvent> {
    events
        .iter()
        .map(|event| HashChainEvent::from_json(event.id.clone(), event).unwrap())
        .collect()
}
