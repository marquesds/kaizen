// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::metrics::types::ToolSpanView;
use rusqlite::{Result, Row};

pub(super) fn tool_span_view_row(row: &Row<'_>) -> Result<ToolSpanView> {
    let mut span = tool_span_identity(row)?;
    apply_span_usage(row, &mut span)?;
    apply_span_hierarchy(row, &mut span)?;
    Ok(span)
}

fn tool_span_identity(row: &Row<'_>) -> Result<ToolSpanView> {
    let paths_json: String = row.get(8)?;
    Ok(ToolSpanView {
        span_id: optional_text(row, 0)?.unwrap_or_default(),
        tool: optional_text(row, 1)?.unwrap_or_else(|| "unknown".into()),
        status: row.get(2)?,
        paths: serde_json::from_str(&paths_json).unwrap_or_default(),
        ..Default::default()
    })
}

fn apply_span_usage(row: &Row<'_>, span: &mut ToolSpanView) -> Result<()> {
    span.lead_time_ms = optional_u64(row, 3)?;
    span.tokens_in = optional_u32(row, 4)?;
    span.tokens_out = optional_u32(row, 5)?;
    span.reasoning_tokens = optional_u32(row, 6)?;
    span.cost_usd_e6 = row.get(7)?;
    Ok(())
}

fn apply_span_hierarchy(row: &Row<'_>, span: &mut ToolSpanView) -> Result<()> {
    span.parent_span_id = row.get(9)?;
    span.depth = optional_u32(row, 10)?.unwrap_or_default();
    span.subtree_cost_usd_e6 = row.get(11)?;
    span.subtree_token_count = optional_u32(row, 12)?;
    Ok(())
}

fn optional_text(row: &Row<'_>, index: usize) -> Result<Option<String>> {
    row.get(index)
}

fn optional_u64(row: &Row<'_>, index: usize) -> Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value as u64))
}

fn optional_u32(row: &Row<'_>, index: usize) -> Result<Option<u32>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value as u32))
}
