use super::{ReportTotalsRead, TokenRead};
use crate::store::Store;
use crate::visualization::{DataQuality, TokenTotals, VisualizationTotals};
use anyhow::Result;

const TOTALS_SQL: &str = "
WITH ws AS (SELECT id, status FROM sessions WHERE workspace = ?1)
SELECT
  (SELECT COUNT(*) FROM ws),
  (SELECT COUNT(*) FROM ws WHERE status IN ('Running', 'Waiting', 'Idle')),
  COUNT(e.id),
  COALESCE(SUM(e.kind = 'Error'), 0),
  COALESCE(SUM(e.kind = 'ToolCall'), 0),
  COALESCE(SUM(e.cost_usd_e6), 0),
  COALESCE(SUM(e.tokens_in), 0),
  COALESCE(SUM(e.tokens_out), 0),
  COALESCE(SUM(e.reasoning_tokens), 0),
  COALESCE(SUM(e.cache_read_tokens), 0),
  COALESCE(SUM(e.cache_creation_tokens), 0),
  COALESCE(SUM(e.id IS NOT NULL AND (e.tokens_in IS NOT NULL OR e.tokens_out IS NOT NULL
    OR e.reasoning_tokens IS NOT NULL OR e.cache_read_tokens IS NOT NULL
    OR e.cache_creation_tokens IS NOT NULL)), 0),
  COUNT(e.cost_usd_e6),
  COUNT(DISTINCT CASE WHEN e.cost_usd_e6 IS NOT NULL THEN e.session_id END)
FROM ws LEFT JOIN events e ON e.session_id = ws.id";

impl Store {
    pub(crate) fn visualization_totals(
        &self,
        workspace: &str,
    ) -> Result<(VisualizationTotals, DataQuality)> {
        let mut statement = self.conn().prepare(TOTALS_SQL)?;
        let row = statement.query_row([workspace], totals_row)?;
        Ok((totals(&row), quality(&row)))
    }
}

fn totals(row: &ReportTotalsRead) -> VisualizationTotals {
    VisualizationTotals {
        session_count: row.session_count,
        running_count: row.running_count,
        event_count: row.event_count,
        error_count: row.error_count,
        tool_call_count: row.tool_call_count,
        cost_usd_e6: row.cost_usd_e6,
        tokens: tokens(row.tokens),
    }
}

fn quality(row: &ReportTotalsRead) -> DataQuality {
    DataQuality {
        token_coverage_pct: pct(row.event_count, row.token_event_count),
        cost_coverage_pct: pct(row.event_count, row.cost_event_count),
        partial_cost_sessions: row.session_count.saturating_sub(row.cost_session_count),
        warnings: warning(row),
        ..Default::default()
    }
}

fn tokens(row: TokenRead) -> TokenTotals {
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

fn pct(total: u64, count: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 * 100.0 / total as f64
    }
}

fn warning(row: &ReportTotalsRead) -> Vec<String> {
    (row.session_count == 0 || row.event_count == 0)
        .then(|| "no local telemetry for workspace".to_string())
        .into_iter()
        .collect()
}

fn totals_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReportTotalsRead> {
    Ok(ReportTotalsRead {
        session_count: value(row, 0)?,
        running_count: value(row, 1)?,
        event_count: value(row, 2)?,
        error_count: value(row, 3)?,
        tool_call_count: value(row, 4)?,
        cost_usd_e6: row.get(5)?,
        tokens: token_row(row)?,
        token_event_count: value(row, 11)?,
        cost_event_count: value(row, 12)?,
        cost_session_count: value(row, 13)?,
    })
}

fn token_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TokenRead> {
    Ok(TokenRead {
        input: value(row, 6)?,
        output: value(row, 7)?,
        reasoning: value(row, 8)?,
        cache_read: value(row, 9)?,
        cache_create: value(row, 10)?,
    })
}

fn value(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    row.get::<_, i64>(index).map(|value| value as u64)
}
