// SPDX-License-Identifier: AGPL-3.0-or-later
//! End-to-end MCP: stdio not required; `duplex` + client `call_tool` exercises the same server handlers.

include!("mcp_tool_names.inc");

use std::env;

use kaizen::core::event::{Event, EventKind, EventSource, SessionRecord, SessionStatus};
use kaizen::mcp::KaizenMcp;
use kaizen::shell::init::init_text;
use kaizen::store::Store;
use rmcp::RoleClient;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use serde_json::json;
use tempfile::tempdir;

fn first_text(r: &CallToolResult) -> String {
    r.content
        .iter()
        .filter_map(|c| c.raw.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_exp_id(created_line: &str) -> &str {
    created_line
        .trim()
        .strip_prefix("created ")
        .and_then(|s| s.split(" ·").next())
        .expect("kaizen_exp_new: expected `created <id> · <name>` line")
        .trim()
}

/// Git repo (metrics), init, session + one event.
fn prepare_workspace() -> anyhow::Result<tempfile::TempDir> {
    let tmp = tempdir()?;
    let ws = tmp.path();
    let g = std::process::Command::new("git")
        .arg("-C")
        .arg(ws)
        .arg("init")
        .status()?;
    anyhow::ensure!(g.success(), "git init");
    std::fs::write(ws.join("README.md"), b"x\n")?;
    let g = std::process::Command::new("git")
        .arg("-C")
        .arg(ws)
        .args(["add", "README.md"])
        .status()?;
    anyhow::ensure!(g.success(), "git add");
    let g = std::process::Command::new("git")
        .arg("-C")
        .arg(ws)
        .args([
            "-c",
            "user.email=x@x",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "c",
        ])
        .status()?;
    anyhow::ensure!(g.success(), "git commit");

    init_text(Some(ws))?;
    let db = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db)?;
    let sid = "sess-mcp-1";
    let wstr = ws.to_string_lossy();
    let session = SessionRecord {
        id: sid.into(),
        agent: "cursor".into(),
        model: Some("m".into()),
        workspace: wstr.to_string(),
        started_at_ms: 1,
        ended_at_ms: None,
        status: SessionStatus::Running,
        trace_path: "".into(),
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
    };
    store.upsert_session(&session)?;
    let ev = Event {
        session_id: sid.into(),
        seq: 0,
        ts_ms: 10,
        ts_exact: true,
        kind: EventKind::ToolCall,
        source: EventSource::Hook,
        tool: Some("bash".into()),
        tool_call_id: Some("c1".into()),
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
        payload: json!({}),
    };
    store.append_event(&ev)?;
    Ok(tmp)
}

async fn tcall(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    args: serde_json::Value,
) -> Result<CallToolResult, rmcp::ServiceError> {
    let map = match args {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    client
        .call_tool(CallToolRequestParams::new(name).with_arguments(map))
        .await
}

#[tokio::test]
async fn every_mcp_tool_runs() -> anyhow::Result<()> {
    let tmp = prepare_workspace()?;
    let w = tmp.path();
    let ws = w.to_str().ok_or_else(|| anyhow::anyhow!("utf8 path"))?;
    let old_cwd = env::current_dir()?;
    env::set_current_dir(w)?;
    let sid = "sess-mcp-1";

    let (server_half, client_half) = tokio::io::duplex(65_536);
    let server = tokio::spawn(async move {
        KaizenMcp.serve(server_half).await?.waiting().await?;
        Ok::<_, anyhow::Error>(())
    });

    let client = ().serve(client_half).await?;
    let tools = client.list_all_tools().await?;
    let mut got: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    got.sort();
    let expected: Vec<_> = KAIZEN_MCP_TOOL_NAMES.to_vec();
    assert_eq!(
        got, expected,
        "MCP list_all_tools must match KAIZEN_MCP_TOOL_NAMES"
    );

    tcall(&client, "kaizen_capabilities", json!({})).await?;
    tcall(&client, "kaizen_init", json!({ "workspace": ws })).await?;
    tcall(
        &client,
        "kaizen_sessions_list",
        json!({ "workspace": ws, "json": true }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_session_show",
        json!({ "workspace": ws, "id": sid }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_summary",
        json!({ "workspace": ws, "json": true }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_insights",
        json!({ "workspace": ws, "refresh": false }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_metrics",
        json!({ "workspace": ws, "json": true, "days": 7 }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_metrics_index",
        json!({ "workspace": ws, "force": false }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_sync_run",
        json!({ "workspace": ws, "once": true }),
    )
    .await?;
    tcall(&client, "kaizen_sync_status", json!({ "workspace": ws })).await?;
    tcall(
        &client,
        "kaizen_retro",
        json!({ "workspace": ws, "json": true, "days": 7, "dry_run": true }),
    )
    .await?;
    let stop_hook = json!({
        "event": "Stop",
        "session_id": sid,
        "stop_reason": "end_turn",
        "timestamp_ms": 1_745_228_800_000_u64
    });
    tcall(
        &client,
        "kaizen_ingest_hook",
        json!({
            "source": "cursor",
            "workspace": ws,
            "payload": stop_hook.to_string()
        }),
    )
    .await?;
    let tui = tcall(&client, "kaizen_tui", json!({ "workspace": ws })).await?;
    assert_eq!(
        tui.is_error,
        Some(true),
        "TUI should report CLI-only / unavailable"
    );
    tcall(
        &client,
        "get_session_span_tree",
        json!({ "workspace": ws, "id": sid, "json": true }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_annotate_session",
        json!({
            "workspace": ws,
            "session_id": sid,
            "score": 3,
            "label": "good",
            "note": "smoke"
        }),
    )
    .await?;

    let r = tcall(
        &client,
        "kaizen_exp_new",
        json!({
            "workspace": ws,
            "name": "mcp-smoke",
            "hypothesis": "h",
            "change": "c",
            "metric": "cost_per_session",
            "bind": "manual"
        }),
    )
    .await?;
    let line = first_text(&r);
    let exp_id = parse_exp_id(&line);

    tcall(&client, "kaizen_exp_list", json!({ "workspace": ws })).await?;
    tcall(
        &client,
        "kaizen_exp_status",
        json!({ "workspace": ws, "id": exp_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_exp_start",
        json!({ "workspace": ws, "id": exp_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_exp_tag",
        json!({
            "workspace": ws,
            "id": exp_id,
            "session": sid,
            "variant": "control"
        }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_exp_report",
        json!({ "workspace": ws, "id": exp_id, "json": true }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_exp_conclude",
        json!({ "workspace": ws, "id": exp_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_exp_archive",
        json!({ "workspace": ws, "id": exp_id }),
    )
    .await?;

    let _ = client.cancel().await;
    server.abort();
    env::set_current_dir(old_cwd)?;
    Ok(())
}
