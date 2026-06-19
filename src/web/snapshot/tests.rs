use super::*;
use crate::core::event::{Event, EventKind, EventSource, SessionRecord};
use crate::core::paths::test_lock;
use crate::visualization::{TraceDetail, TraceSummary};
use serde_json::json;
use std::ffi::OsString;

mod bounds;

#[test]
fn compact_report_strips_web_payload() {
    let mut report = report();
    compact_report(&mut report);
    assert_eq!(report.sessions.len(), 31);
    let detail = report.selected.unwrap();
    assert_eq!(detail.events.len(), 41);
    assert!(detail.events.iter().all(|event| event.payload.is_null()));
    assert!(report.activity.day_bins.is_empty());
    assert!(report.activity.week_bins.is_empty());
    assert_eq!(report.totals.session_count, 31);
}

#[test]
fn compact_report_keeps_prompt_and_command_summary() {
    let mut report = report();
    let detail = report.selected.as_mut().unwrap();
    detail.events[0].kind = EventKind::Lifecycle;
    detail.events[0].payload = json!({"type":"user_prompt_submit","prompt":"Fix the parser"});
    detail.events[1].kind = EventKind::ToolCall;
    detail.events[1].tool = Some("Bash".into());
    detail.events[1].payload = json!({"tool_input":{"command":"rg parser src"}});
    compact_report(&mut report);
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(
        value.pointer("/selected/prompt"),
        Some(&json!("Fix the parser"))
    );
    assert_eq!(
        value.pointer("/selected/events/1/payload/summary"),
        Some(&json!("rg parser src"))
    );
}

#[test]
fn missing_workspace_does_not_create_project_state() {
    let _guard = test_lock::global().lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let _env = HomeGuard::set(&home);
    let request = SnapshotRequest {
        workspace: temp.path().join("missing").display().to_string(),
        selected_session_id: None,
    };
    assert!(load(request).is_err());
    assert!(!home.join("projects").exists());
}

#[test]
fn uninitialized_workspace_does_not_create_project_state() {
    let _guard = test_lock::global().lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let workspace = temp.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let _env = HomeGuard::set(&home);
    let request = SnapshotRequest {
        workspace: workspace.display().to_string(),
        selected_session_id: None,
    };
    let error = load(request).unwrap_err().to_string();
    assert!(error.contains("no Kaizen data"), "{error}");
    assert!(!home.join("projects").exists());
}

fn report() -> VisualizationReport {
    let session = session("s0");
    VisualizationReport {
        totals: crate::visualization::VisualizationTotals {
            session_count: 31,
            ..Default::default()
        },
        sessions: (0..31).map(|n| summary(&format!("s{n}"))).collect(),
        selected: Some(TraceDetail {
            session,
            prompt: None,
            events: (0..41).map(event).collect(),
            spans: Vec::new(),
            files: Vec::new(),
        }),
        activity: crate::visualization::ActivityReport {
            day_bins: vec![Default::default()],
            week_bins: vec![Default::default()],
            ..Default::default()
        },
        ..Default::default()
    }
}

fn summary(id: &str) -> TraceSummary {
    serde_json::from_value(json!({
        "id": id, "agent": "codex", "model": null, "status": "idle",
        "status_reason": "test", "started_at_ms": 1, "ended_at_ms": null,
        "last_event_ms": null, "event_count": 0, "error_count": 0,
        "tool_call_count": 0, "cost_usd_e6": 0,
        "tokens": {"input": 0, "output": 0, "reasoning": 0,
            "cache_read": 0, "cache_create": 0, "total": 0},
        "top_tools": []
    }))
    .unwrap()
}

fn session(id: &str) -> SessionRecord {
    serde_json::from_value(json!({
        "id": id, "agent": "codex", "model": null, "workspace": "/tmp",
        "started_at_ms": 1, "ended_at_ms": null, "status": "Running",
        "trace_path": "", "start_commit": null, "end_commit": null,
        "branch": null, "dirty_start": null, "dirty_end": null,
        "repo_binding_source": null, "prompt_fingerprint": null,
        "parent_session_id": null, "agent_version": null, "os": null,
        "arch": null, "repo_file_count": null, "repo_total_loc": null
    }))
    .unwrap()
}

fn event(seq: u64) -> Event {
    Event {
        session_id: "s0".into(),
        seq,
        ts_ms: seq,
        ts_exact: true,
        kind: EventKind::Hook,
        source: EventSource::Hook,
        tool: None,
        tool_call_id: None,
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: json!({"large": "payload"}),
    }
}

struct HomeGuard(Option<OsString>);

impl HomeGuard {
    fn set(path: &PathBuf) -> Self {
        let previous = std::env::var_os("KAIZEN_HOME");
        unsafe { std::env::set_var("KAIZEN_HOME", path) };
        Self(previous)
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match self.0.take() {
            Some(value) => unsafe { std::env::set_var("KAIZEN_HOME", value) },
            None => unsafe { std::env::remove_var("KAIZEN_HOME") },
        }
    }
}
