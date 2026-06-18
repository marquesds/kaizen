use super::{ActivityCountRead, ActivityRead, ActivityTotalRead, ActivityWindow};
use crate::store::Store;
use anyhow::Result;
use rusqlite::params;

const TOTALS_SQL: &str = "
SELECT CAST((e.ts_ms - ?2) / ?4 AS INTEGER), COUNT(*), COUNT(DISTINCT e.session_id),
 COALESCE(SUM(COALESCE(e.tokens_in, 0) + COALESCE(e.tokens_out, 0)
  + COALESCE(e.reasoning_tokens, 0) + COALESCE(e.cache_read_tokens, 0)
  + COALESCE(e.cache_creation_tokens, 0)), 0), COALESCE(SUM(e.cost_usd_e6), 0)
FROM events e JOIN sessions s ON s.id = e.session_id
WHERE s.workspace = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3
GROUP BY 1 ORDER BY 1";

const AGENTS_SQL: &str = "
SELECT CAST((e.ts_ms - ?2) / ?4 AS INTEGER), s.agent, COUNT(*)
FROM events e JOIN sessions s ON s.id = e.session_id
WHERE s.workspace = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3
GROUP BY 1, s.agent ORDER BY 1, 3 DESC, 2 ASC";

const KINDS_SQL: &str = "
SELECT CAST((e.ts_ms - ?2) / ?4 AS INTEGER),
 CASE e.kind WHEN 'ToolCall' THEN 'tool_call' WHEN 'ToolResult' THEN 'tool_result'
  WHEN 'Message' THEN 'message' WHEN 'Error' THEN 'error' WHEN 'Cost' THEN 'cost'
  WHEN 'Lifecycle' THEN 'lifecycle' ELSE 'hook' END, COUNT(*)
FROM events e JOIN sessions s ON s.id = e.session_id
WHERE s.workspace = ?1 AND e.ts_ms >= ?2 AND e.ts_ms < ?3
GROUP BY 1, 2 ORDER BY 1, 3 DESC, 2 ASC";

impl Store {
    pub(crate) fn visualization_activity(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
        width_ms: u64,
    ) -> Result<ActivityRead> {
        let window = ActivityWindow {
            start_ms,
            end_ms,
            width_ms,
        };
        Ok(ActivityRead {
            totals: totals(self, workspace, window)?,
            agents: counts(self, AGENTS_SQL, workspace, window)?,
            kinds: counts(self, KINDS_SQL, workspace, window)?,
        })
    }
}

fn totals(
    store: &Store,
    workspace: &str,
    window: ActivityWindow,
) -> Result<Vec<ActivityTotalRead>> {
    let mut statement = store.conn().prepare(TOTALS_SQL)?;
    let rows = statement.query_map(
        params![
            workspace,
            window.start_ms as i64,
            window.end_ms as i64,
            window.width_ms as i64
        ],
        total_row,
    )?;
    rows.map(|row| row.map_err(Into::into)).collect()
}

fn counts(
    store: &Store,
    sql: &str,
    workspace: &str,
    window: ActivityWindow,
) -> Result<Vec<ActivityCountRead>> {
    let mut statement = store.conn().prepare(sql)?;
    let rows = statement.query_map(
        params![
            workspace,
            window.start_ms as i64,
            window.end_ms as i64,
            window.width_ms as i64
        ],
        count_row,
    )?;
    rows.map(|row| row.map_err(Into::into)).collect()
}

fn total_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityTotalRead> {
    Ok(ActivityTotalRead {
        bin: value(row, 0)? as usize,
        event_count: value(row, 1)?,
        session_count: value(row, 2)?,
        token_total: value(row, 3)?,
        cost_usd_e6: row.get(4)?,
    })
}

fn count_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityCountRead> {
    Ok(ActivityCountRead {
        bin: value(row, 0)? as usize,
        name: row.get(1)?,
        count: value(row, 2)?,
    })
}

fn value(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    row.get::<_, i64>(index).map(|value| value as u64)
}
