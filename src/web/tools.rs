// SPDX-License-Identifier: AGPL-3.0-or-later
//! Implemented Web actions reuse MCP handlers in-process.

use crate::mcp::KaizenMcp;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde::Serialize;
use serde_json::{Map, Value};

pub const WEB_TOOL_NAMES: &[&str] = &["kaizen_sessions_list"];

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
    reject_refresh_scan(&args)?;
    call_mcp(name, args_map(args)?).await.and_then(output)
}

fn reject_refresh_scan(args: &Value) -> Result<(), String> {
    match args.get("refresh").and_then(Value::as_bool) {
        Some(true) => Err("web Observe does not allow refresh scans".into()),
        _ => Ok(()),
    }
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
        None => parsed_json(&text).map_or(ToolOutput::Text(text), ToolOutput::Json),
    })
}

fn parsed_json(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn result_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.raw.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}
