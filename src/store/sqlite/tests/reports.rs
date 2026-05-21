use super::super::{GuidanceKind, Store};
use super::{make_event, make_session};
use serde_json::json;
use std::collections::HashSet;
use tempfile::TempDir;

#[test]
fn summary_stats_empty() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    let stats = store.summary_stats("/ws").unwrap();
    assert_eq!(stats.session_count, 0);
    assert_eq!(stats.total_cost_usd_e6, 0);
}

#[test]
fn summary_stats_counts_sessions() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("a")).unwrap();
    store.upsert_session(&make_session("b")).unwrap();
    let stats = store.summary_stats("/ws").unwrap();
    assert_eq!(stats.session_count, 2);
    assert_eq!(stats.by_agent.len(), 1);
    assert_eq!(stats.by_agent[0].0, "cursor");
    assert_eq!(stats.by_agent[0].1, 2);
}

#[test]
fn guidance_report_counts_skill_and_rule_sessions() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("kaizen.db")).unwrap();
    store.upsert_session(&make_session("sx")).unwrap();
    let mut ev = make_event("sx", 0);
    ev.payload = json!({"text": "read .cursor/skills/tdd/SKILL.md and .cursor/rules/style.mdc"});
    ev.cost_usd_e6 = Some(500_000);
    store.append_event(&ev).unwrap();

    let mut skill_slugs = HashSet::new();
    skill_slugs.insert("tdd".into());
    let mut rule_slugs = HashSet::new();
    rule_slugs.insert("style".into());

    let rep = store
        .guidance_report("/ws", 0, 10_000, &skill_slugs, &rule_slugs)
        .unwrap();
    assert_eq!(rep.sessions_in_window, 1);
    let tdd = rep
        .rows
        .iter()
        .find(|r| r.id == "tdd" && r.kind == GuidanceKind::Skill)
        .unwrap();
    assert_eq!(tdd.sessions, 1);
    assert!(tdd.on_disk);
    let style = rep
        .rows
        .iter()
        .find(|r| r.id == "style" && r.kind == GuidanceKind::Rule)
        .unwrap();
    assert_eq!(style.sessions, 1);
    assert!(style.on_disk);
}
