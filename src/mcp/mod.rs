// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model Context Protocol (stdio) — full CLI parity.

mod handler;

use anyhow::Result;
use handler::KaizenMcp;
use rmcp::ServiceExt;
use rmcp::transport::stdio;

/// Run the MCP server on stdin/stdout until the client disconnects.
pub async fn run_stdio_server() -> Result<()> {
    let (r, w) = stdio();
    let _ = KaizenMcp.serve((r, w)).await?.waiting().await?;
    Ok(())
}
