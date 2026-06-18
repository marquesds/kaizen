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
fn prepare_workspace() -> anyhow::Result<(tempfile::TempDir, tempfile::TempDir)> {
    let home = tempdir()?;
    let tmp = tempdir()?;
    let ws = tmp.path();
    unsafe { std::env::set_var("KAIZEN_HOME", home.path()) };
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
    let store = Store::open(&kaizen::core::workspace::db_path(ws)?)?;
    let sid = "sess-mcp-1";
    let canonical = kaizen::core::workspace::canonical(ws);
    let wstr = canonical.to_string_lossy();
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
    Ok((home, tmp))
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
    let (_home, tmp) = prepare_workspace()?;
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
        "kaizen_search_sessions",
        json!({ "workspace": ws, "query": "bash", "limit": 5 }),
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
    tcall(
        &client,
        "kaizen_query",
        json!({ "workspace": ws, "expr": "tool:bash", "since": "99999d" }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_cases_create",
        json!({
            "workspace": ws,
            "session_id": sid,
            "reason": "smoke",
            "label": "manual"
        }),
    )
    .await?;
    tcall(&client, "kaizen_cases_mine", json!({ "workspace": ws })).await?;
    tcall(&client, "kaizen_cases_list", json!({ "workspace": ws })).await?;
    let store = Store::open(&kaizen::core::workspace::db_path(w)?)?;
    let case_id = kaizen::core_loop::cases::list(&store, None)?[0].id.clone();
    tcall(
        &client,
        "kaizen_cases_show",
        json!({ "workspace": ws, "id": case_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_cases_archive",
        json!({ "workspace": ws, "id": case_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_rules_create",
        json!({
            "workspace": ws,
            "name": "smoke-review",
            "filter": "tool:bash",
            "action": "queue_review",
            "message": "review smoke"
        }),
    )
    .await?;
    tcall(&client, "kaizen_rules_list", json!({ "workspace": ws })).await?;
    let rule_id = kaizen::core_loop::rules::list(&store)?[0].id.clone();
    tcall(
        &client,
        "kaizen_rules_disable",
        json!({ "workspace": ws, "id": rule_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_rules_enable",
        json!({ "workspace": ws, "id": rule_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_rules_run",
        json!({ "workspace": ws, "since": "99999d" }),
    )
    .await?;
    tcall(&client, "kaizen_alerts_check", json!({ "workspace": ws })).await?;
    tcall(&client, "kaizen_review_list", json!({ "workspace": ws })).await?;
    let reviews = kaizen::core_loop::review::list(&store, None)?;
    let review_id = match reviews.first() {
        Some(row) => row.id.clone(),
        None => {
            kaizen::core_loop::review::create(&store, "smoke-review", sid, "review smoke", 1)?.id
        }
    };
    tcall(
        &client,
        "kaizen_review_show",
        json!({ "workspace": ws, "id": review_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_review_resolve",
        json!({ "workspace": ws, "id": review_id }),
    )
    .await?;
    tcall(
        &client,
        "kaizen_review_dismiss",
        json!({ "workspace": ws, "id": review_id }),
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
