use super::session_search::{self, SessionRowsPage, SessionSearchQuery};
use super::{SessionSummaryRead, TokenRead};
use crate::store::Store;
use crate::visualization::{SessionPageMeta, TokenTotals, TraceSummary, derive_status};
use anyhow::Result;
use rusqlite::params_from_iter;
use std::collections::HashMap;

const TOP_TOOLS_SUFFIX: &str = ") AND t.tool <> ''
 GROUP BY t.session_id, t.tool
), ranked AS (
 SELECT session_id, tool, count, ROW_NUMBER() OVER (
  PARTITION BY session_id ORDER BY count DESC, tool ASC) rank FROM counts
)
SELECT session_id, tool, count FROM ranked WHERE rank <= 5 ORDER BY session_id, rank";

impl Store {
    pub(crate) fn visualization_sessions(
        &self,
        query: &SessionSearchQuery,
        now_ms: u64,
    ) -> Result<(Vec<TraceSummary>, SessionPageMeta)> {
        let mut page = session_search::read(self, query)?;
        attach_tools(self, &mut page.rows)?;
        let meta = page_meta(&page);
        Ok((
            page.rows
                .into_iter()
                .map(|row| summary(row, now_ms))
                .collect(),
            meta,
        ))
    }
}

fn attach_tools(store: &Store, rows: &mut [SessionSummaryRead]) -> Result<()> {
    let ids = rows
        .iter()
        .map(|row| row.session.id.as_str())
        .collect::<Vec<_>>();
    let mut tools = top_tools(store, &ids)?;
    rows.iter_mut()
        .for_each(|row| row.top_tools = tools.remove(&row.session.id).unwrap_or_default());
    Ok(())
}

fn top_tools(store: &Store, ids: &[&str]) -> Result<HashMap<String, Vec<(String, u64)>>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut statement = store.conn().prepare(&top_tools_sql(ids.len()))?;
    let rows = statement.query_map(params_from_iter(ids), tool_row)?;
    let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows.into_iter().fold(HashMap::new(), add_tool))
}

fn top_tools_sql(count: usize) -> String {
    let slots = (1..=count)
        .map(|n| format!("?{n}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "WITH counts AS (SELECT t.session_id, t.tool, COUNT(*) count FROM tool_spans t WHERE t.session_id IN ({slots}{TOP_TOOLS_SUFFIX}"
    )
}

fn page_meta(page: &SessionRowsPage) -> SessionPageMeta {
    let shown = page.offset.saturating_add(page.rows.len());
    SessionPageMeta {
        filtered_total: page.filtered_total,
        offset: page.offset,
        limit: page.limit,
        next_offset: (shown < page.filtered_total).then_some(shown),
    }
}

fn add_tool(
    mut tools: HashMap<String, Vec<(String, u64)>>,
    row: (String, String, u64),
) -> HashMap<String, Vec<(String, u64)>> {
    tools.entry(row.0).or_default().push((row.1, row.2));
    tools
}

fn tool_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, u64)> {
    Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as u64))
}

pub(super) fn summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummaryRead> {
    Ok(SessionSummaryRead {
        session: super::super::rows::session_row(row)?,
        last_event_ms: optional(row, 21)?,
        event_count: value(row, 22)?,
        error_count: value(row, 23)?,
        tool_call_count: value(row, 24)?,
        cost_usd_e6: row.get(25)?,
        tokens: tokens(row)?,
        top_tools: Vec::new(),
    })
}

fn tokens(row: &rusqlite::Row<'_>) -> rusqlite::Result<TokenRead> {
    Ok(TokenRead {
        input: value(row, 26)?,
        output: value(row, 27)?,
        reasoning: value(row, 28)?,
        cache_read: value(row, 29)?,
        cache_create: value(row, 30)?,
    })
}

fn optional(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)
        .map(|value| value.map(|value| value as u64))
}

fn value(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    row.get::<_, i64>(index).map(|value| value as u64)
}

fn summary(row: SessionSummaryRead, now_ms: u64) -> TraceSummary {
    let (status, status_reason) =
        derive_status(&row.session, row.last_event_ms, row.error_count, now_ms);
    TraceSummary {
        id: row.session.id,
        agent: row.session.agent,
        model: row.session.model,
        status,
        status_reason,
        started_at_ms: row.session.started_at_ms,
        ended_at_ms: row.session.ended_at_ms,
        last_event_ms: row.last_event_ms,
        event_count: row.event_count,
        error_count: row.error_count,
        tool_call_count: row.tool_call_count,
        cost_usd_e6: row.cost_usd_e6,
        tokens: token_totals(row.tokens),
        top_tools: row.top_tools,
    }
}

fn token_totals(row: TokenRead) -> TokenTotals {
    let total = row.input + row.output + row.reasoning + row.cache_read + row.cache_create;
    TokenTotals {
        input: row.input,
        output: row.output,
        reasoning: row.reasoning,
        cache_read: row.cache_read,
        cache_create: row.cache_create,
        total,
    }
}
