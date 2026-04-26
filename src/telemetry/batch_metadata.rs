// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pure summary JSON for local NDJSON telemetry (no full payloads).

use crate::sync::IngestExportBatch;
use crate::sync::canonical::KAIZEN_SCHEMA_VERSION;
use crate::sync::outbound::EventsBatchBody;
use crate::sync::smart::{RepoSnapshotsBatchBody, ToolSpansBatchBody};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::collections::HashSet;

const TALLY_CAP: usize = 20;

/// One NDJSON object per batch. `emitted_at_ms` set at file write in [`super::file::FileExporter`].
pub fn telemetry_file_line(batch: &IngestExportBatch, emitted_at_ms: i64) -> Value {
    match batch {
        IngestExportBatch::Events(b) => {
            let mut o = base_envelope(
                "events",
                b.events.len(),
                b.team_id.as_str(),
                b.workspace_hash.as_str(),
                emitted_at_ms,
            );
            merge_events(&mut o, b);
            Value::Object(o)
        }
        IngestExportBatch::ToolSpans(b) => {
            let mut o = base_envelope(
                "tool_spans",
                b.spans.len(),
                b.team_id.as_str(),
                b.workspace_hash.as_str(),
                emitted_at_ms,
            );
            merge_tool_spans(&mut o, b);
            Value::Object(o)
        }
        IngestExportBatch::RepoSnapshots(b) => {
            let mut o = base_envelope(
                "repo_snapshots",
                b.snapshots.len(),
                b.team_id.as_str(),
                b.workspace_hash.as_str(),
                emitted_at_ms,
            );
            merge_repo(&mut o, b);
            Value::Object(o)
        }
        IngestExportBatch::WorkspaceFacts(b) => {
            let o = base_envelope(
                "workspace_facts",
                b.facts.len(),
                b.team_id.as_str(),
                b.workspace_hash.as_str(),
                emitted_at_ms,
            );
            Value::Object(o)
        }
        IngestExportBatch::SessionEvals(b) => {
            body_only("session_evals", b.evals.len(), emitted_at_ms)
        }
        IngestExportBatch::SessionFeedback(b) => {
            body_only("session_feedback", b.feedback.len(), emitted_at_ms)
        }
    }
}

fn body_only(kind: &str, item_count: usize, t: i64) -> Value {
    json!({
        "kaizen_schema_version": KAIZEN_SCHEMA_VERSION,
        "batch_kind": kind,
        "item_count": item_count,
        "emitted_at_ms": t,
    })
}

fn base_envelope(
    kind: &str,
    item_count: usize,
    team: &str,
    wh: &str,
    t: i64,
) -> serde_json::Map<String, Value> {
    let mut o = serde_json::Map::new();
    o.insert("kaizen_schema_version".into(), KAIZEN_SCHEMA_VERSION.into());
    o.insert("batch_kind".into(), kind.into());
    o.insert("item_count".into(), (item_count as u64).into());
    o.insert("emitted_at_ms".into(), t.into());
    o.insert("team_id".into(), team.into());
    o.insert("workspace_hash".into(), wh.into());
    o
}

fn merge_events(map: &mut serde_json::Map<String, Value>, b: &EventsBatchBody) {
    let n_sess = b
        .events
        .iter()
        .map(|e| e.session_id_hash.as_str())
        .collect::<HashSet<_>>()
        .len();
    map.insert("session_id_count".into(), n_sess.into());
    let (mn, mx) = ts_min_max(b.events.iter().map(|e| e.ts_ms));
    if let Some(v) = mn {
        map.insert("ts_ms_min".into(), v.into());
    }
    if let Some(v) = mx {
        map.insert("ts_ms_max".into(), v.into());
    }
    map.insert(
        "kind_tally".into(),
        capped_tally(b.events.iter().map(|e| e.kind.as_str())),
    );
    map.insert(
        "source_tally".into(),
        capped_tally(b.events.iter().map(|e| e.source.as_str())),
    );
}

fn ts_min_max<I: Iterator<Item = u64>>(it: I) -> (Option<u64>, Option<u64>) {
    let mut it = it;
    let first = it.next();
    let (mut mn, mut mx) = (first, first);
    for t in it {
        mn = Some(mn.map_or(t, |m| m.min(t)));
        mx = Some(mx.map_or(t, |m| m.max(t)));
    }
    (mn, mx)
}

fn capped_tally<'a>(it: impl Iterator<Item = &'a str>) -> Value {
    let mut m: BTreeMap<String, u64> = BTreeMap::new();
    for s in it {
        *m.entry(s.to_string()).or_insert(0) += 1;
    }
    let mut out = serde_json::Map::new();
    for (k, v) in m.into_iter().take(TALLY_CAP) {
        out.insert(k, v.into());
    }
    Value::Object(out)
}

fn merge_tool_spans(map: &mut serde_json::Map<String, Value>, b: &ToolSpansBatchBody) {
    let n_sess = b
        .spans
        .iter()
        .map(|s| s.session_id_hash.as_str())
        .collect::<HashSet<_>>()
        .len();
    map.insert("session_id_count".into(), n_sess.into());
    let starts = b.spans.iter().filter_map(|s| s.started_at_ms);
    let (mn, mx) = ts_min_max(starts);
    if let Some(v) = mn {
        map.insert("started_at_ms_min".into(), v.into());
    }
    if let Some(v) = mx {
        map.insert("started_at_ms_max".into(), v.into());
    }
}

fn merge_repo(map: &mut serde_json::Map<String, Value>, b: &RepoSnapshotsBatchBody) {
    if b.snapshots.is_empty() {
        return;
    }
    let (mn, mx) = ts_min_max(b.snapshots.iter().map(|s| s.indexed_at_ms));
    if let Some(v) = mn {
        map.insert("indexed_at_ms_min".into(), v.into());
    }
    if let Some(v) = mx {
        map.insert("indexed_at_ms_max".into(), v.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::export_batch::{SessionEvalsBatchBody, SessionFeedbackBatchBody};
    use crate::sync::outbound::OutboundEvent;

    fn v_str(v: &Value, k: &str) -> String {
        v.get(k).and_then(|x| x.as_str()).unwrap().to_string()
    }

    fn v_u64(v: &Value, k: &str) -> u64 {
        v.get(k).and_then(|x| x.as_u64()).unwrap()
    }

    #[test]
    fn session_evals_body_only() {
        let b = IngestExportBatch::SessionEvals(SessionEvalsBatchBody { evals: vec![] });
        let j = telemetry_file_line(&b, 42);
        assert_eq!(v_str(&j, "batch_kind"), "session_evals");
        assert_eq!(v_u64(&j, "item_count"), 0);
        assert!(j.get("team_id").is_none());
    }

    #[test]
    fn events_envelope_and_rollups() {
        let ev = |ts: u64, kind: &str, src: &str, sid: &str| OutboundEvent {
            session_id_hash: sid.into(),
            event_seq: 0,
            ts_ms: ts,
            agent: "a".into(),
            model: "m".into(),
            kind: kind.into(),
            source: src.into(),
            tool: None,
            tool_call_id: None,
            tokens_in: None,
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: None,
            payload: json!({}),
        };
        let b = IngestExportBatch::Events(EventsBatchBody {
            team_id: "t1".into(),
            workspace_hash: "wh1".into(),
            events: vec![ev(10, "x", "tail", "s1"), ev(20, "x", "hook", "s2")],
        });
        let j = telemetry_file_line(&b, 0);
        assert_eq!(v_str(&j, "team_id"), "t1");
        assert_eq!(v_u64(&j, "session_id_count"), 2);
        assert_eq!(v_u64(&j, "ts_ms_min"), 10);
        assert_eq!(v_u64(&j, "ts_ms_max"), 20);
    }

    #[test]
    fn session_feedback_body_only() {
        let b = IngestExportBatch::SessionFeedback(SessionFeedbackBatchBody { feedback: vec![] });
        let j = telemetry_file_line(&b, 0);
        assert_eq!(v_str(&j, "batch_kind"), "session_feedback");
        assert!(j.get("team_id").is_none());
    }
}
