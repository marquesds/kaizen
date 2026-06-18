// SPDX-License-Identifier: AGPL-3.0-or-later
//! Regression coverage: read surfaces must not initialize persistent state.

use kaizen::mcp::KaizenMcp;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::json;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};

struct CliCase {
    name: &'static str,
    args: &'static [&'static str],
    success: bool,
    output: &'static str,
}

struct KaizenHomeGuard(Option<OsString>);

impl KaizenHomeGuard {
    fn set(path: &Path) -> Self {
        let previous = std::env::var_os("KAIZEN_HOME");
        unsafe { std::env::set_var("KAIZEN_HOME", path) };
        Self(previous)
    }
}

impl Drop for KaizenHomeGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            unsafe { std::env::set_var("KAIZEN_HOME", previous) };
        } else {
            unsafe { std::env::remove_var("KAIZEN_HOME") };
        }
    }
}

const CLI_CASES: &[CliCase] = &[
    CliCase {
        name: "doctor",
        args: &["doctor"],
        success: true,
        output: "store: OK",
    },
    CliCase {
        name: "show",
        args: &["sessions", "show", "missing"],
        success: false,
        output: "session not found: missing — try `kaizen sessions list`",
    },
    CliCase {
        name: "tree",
        args: &["sessions", "tree", "missing"],
        success: false,
        output: "session not found: missing",
    },
    CliCase {
        name: "trace",
        args: &["sessions", "trace", "missing"],
        success: false,
        output: "session not found: missing",
    },
    CliCase {
        name: "query",
        args: &["query", "tool:bash", "--json"],
        success: true,
        output: "[]",
    },
];

fn run_cli(home: &Path, workspace: &Path, args: &[&str]) -> anyhow::Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_kaizen"))
        .args(args)
        .current_dir(workspace)
        .env("HOME", home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .output()?)
}

fn command_output(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_expected(case: &CliCase, output: &Output) {
    let text = command_output(output);
    assert_eq!(
        output.status.success(),
        case.success,
        "{}: {text}",
        case.name
    );
    assert!(text.contains(case.output), "{}: {text}", case.name);
}

fn assert_cli_case(root: &Path, case: &CliCase) -> anyhow::Result<()> {
    let home = root.join(case.name);
    let workspace = root.join(format!("{}-workspace", case.name));
    std::fs::create_dir(&workspace)?;
    let output = run_cli(&home, &workspace, case.args)?;
    assert_expected(case, &output);
    assert!(
        !home.join(".kaizen").exists(),
        "{} created state",
        case.name
    );
    Ok(())
}

#[test]
fn cli_read_surfaces_leave_fresh_home_untouched() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    CLI_CASES
        .iter()
        .try_for_each(|case| assert_cli_case(root.path(), case))
}

async fn call_query(
    client: &RunningService<RoleClient, ()>,
    workspace: &Path,
) -> Result<CallToolResult, rmcp::ServiceError> {
    let args = json!({
        "workspace": workspace.to_string_lossy(),
        "expr": "tool:bash",
        "limit": 5
    });
    let map = args.as_object().cloned().unwrap_or_default();
    client
        .call_tool(CallToolRequestParams::new("kaizen_query").with_arguments(map))
        .await
}

fn result_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|item| item.raw.as_text().map(|text| text.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_empty_query(result: &CallToolResult) -> anyhow::Result<()> {
    let value: serde_json::Value = serde_json::from_str(&result_text(result))?;
    assert_eq!(value["count"], 0);
    assert_eq!(value["hits"], json!([]));
    Ok(())
}

async fn run_mcp_query(workspace: &Path) -> anyhow::Result<CallToolResult> {
    let (server_io, client_io) = tokio::io::duplex(65_536);
    let server = tokio::spawn(async move {
        KaizenMcp.serve(server_io).await?.waiting().await?;
        Ok::<_, anyhow::Error>(())
    });
    let client = ().serve(client_io).await?;
    let result = call_query(&client, workspace).await?;
    let _ = client.cancel().await;
    server.abort();
    Ok(result)
}

#[tokio::test]
async fn mcp_query_leaves_fresh_home_untouched() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let home = root.path().join("home");
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace)?;
    let _home = KaizenHomeGuard::set(&home.join(".kaizen"));
    let result = run_mcp_query(&workspace).await?;
    assert_empty_query(&result)?;
    assert!(!home.join(".kaizen").exists(), "MCP query created state");
    Ok(())
}
