// SPDX-License-Identifier: AGPL-3.0-or-later
//! Web tool registry. Names mirror MCP; execution reuses MCP handlers in-process.

use crate::mcp::KaizenMcp;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde::Serialize;
use serde_json::{Map, Value, json};

pub const WEB_TOOL_NAMES: &[&str] = &[
    "get_session_span_tree",
    "kaizen_alerts_check",
    "kaizen_annotate_session",
    "kaizen_capabilities",
    "kaizen_cases_archive",
    "kaizen_cases_create",
    "kaizen_cases_list",
    "kaizen_cases_mine",
    "kaizen_cases_show",
    "kaizen_exp_archive",
    "kaizen_exp_conclude",
    "kaizen_exp_list",
    "kaizen_exp_new",
    "kaizen_exp_report",
    "kaizen_exp_start",
    "kaizen_exp_status",
    "kaizen_exp_tag",
    "kaizen_ingest_hook",
    "kaizen_init",
    "kaizen_insights",
    "kaizen_metrics",
    "kaizen_metrics_index",
    "kaizen_query",
    "kaizen_retro",
    "kaizen_review_dismiss",
    "kaizen_review_list",
    "kaizen_review_resolve",
    "kaizen_review_show",
    "kaizen_rules_create",
    "kaizen_rules_disable",
    "kaizen_rules_enable",
    "kaizen_rules_list",
    "kaizen_rules_run",
    "kaizen_session_show",
    "kaizen_sessions_list",
    "kaizen_summary",
    "kaizen_sync_run",
    "kaizen_sync_status",
    "kaizen_tui",
    "mcp/search_sessions",
];

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ToolOutput {
    Json(Value),
    Text(String),
}

pub async fn call(name: &str, args: Value) -> Result<ToolOutput, String> {
    if !WEB_TOOL_NAMES.contains(&name) {
        return Err(format!("unknown web tool: {name}"));
    }
    if name == "kaizen_tui" {
        return Ok(ToolOutput::Json(json!({
            "available": true,
            "feature": "browser_live_session_view"
        })));
    }
    call_mcp(name, args_map(args)?).await.and_then(output)
}

async fn call_mcp(name: &str, args: Map<String, Value>) -> Result<CallToolResult, String> {
    let (server_half, client_half) = tokio::io::duplex(1_048_576);
    let server = tokio::spawn(async move {
        let svc = KaizenMcp.serve(server_half).await?;
        svc.waiting().await?;
        Ok::<_, anyhow::Error>(())
    });
    let client = ().serve(client_half).await.map_err(|e| e.to_string())?;
    let params = CallToolRequestParams::new(name.to_string()).with_arguments(args);
    let result = client.call_tool(params).await.map_err(|e| e.to_string());
    drop(client);
    server.abort();
    result
}

fn args_map(args: Value) -> Result<Map<String, Value>, String> {
    match args {
        Value::Null => Ok(Map::new()),
        Value::Object(map) => Ok(map),
        _ => Err("tool args must be a JSON object".into()),
    }
}

fn output(result: CallToolResult) -> Result<ToolOutput, String> {
    let text = result_text(&result);
    if result.is_error == Some(true) {
        return Err(text);
    }
    Ok(match result.structured_content {
        Some(value) => ToolOutput::Json(value),
        None => ToolOutput::Text(text),
    })
}

fn result_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.raw.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}
