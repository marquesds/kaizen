const ORDINARY_CANDIDATES: &str = "WITH candidates AS MATERIALIZED (
 SELECT s.id, s.agent, s.model, s.branch, s.started_at_ms,
  COALESCE(p.prompt, '') prompt
 FROM sessions s
 LEFT JOIN session_search_prompts p ON p.session_id = s.id
 WHERE s.workspace = :workspace
)";

const STATUS_CANDIDATES: &str = "WITH status_rollup AS MATERIALIZED (
 SELECT e.session_id, MAX(e.ts_ms) last_event_ms,
  SUM(e.kind = 'Error') error_count
 FROM events e JOIN sessions w ON w.id = e.session_id
 WHERE w.workspace = :workspace GROUP BY e.session_id
), candidates AS MATERIALIZED (
 SELECT s.id, s.agent, s.model, s.branch, s.started_at_ms,
  COALESCE(p.prompt, '') prompt,
  CASE
   WHEN COALESCE(r.error_count, 0) > 0 THEN 'errored'
   WHEN s.status = 'Done' OR s.ended_at_ms IS NOT NULL THEN 'done'
   WHEN r.last_event_ms IS NULL THEN 'idle'
   WHEN :now_ms - r.last_event_ms <= 300000 THEN 'active'
   WHEN :now_ms - r.last_event_ms >= 1800000 THEN 'orphaned'
   ELSE 'idle'
  END derived_status
 FROM sessions s
 LEFT JOIN session_search_prompts p ON p.session_id = s.id
 LEFT JOIN status_rollup r ON r.session_id = s.id
 WHERE s.workspace = :workspace
)";

const MATCH_PREFIX: &str = "(:text = ''
 OR instr(kaizen_casefold(c.prompt), kaizen_casefold(:text)) > 0
 OR instr(kaizen_casefold(c.id), kaizen_casefold(:text)) > 0
 OR instr(kaizen_casefold(c.agent), kaizen_casefold(:text)) > 0
 OR instr(kaizen_casefold(COALESCE(c.model, '')), kaizen_casefold(:text)) > 0
 OR instr(kaizen_casefold(COALESCE(c.branch, '')), kaizen_casefold(:text)) > 0";

const TOOL_MATCHES: &str = "
 OR EXISTS (SELECT 1 FROM events e WHERE e.session_id = c.id
  AND instr(kaizen_casefold(COALESCE(e.tool, '')), kaizen_casefold(:text)) > 0)
 OR EXISTS (SELECT 1 FROM tool_spans t WHERE t.session_id = c.id
  AND instr(kaizen_casefold(COALESCE(t.tool, '')), kaizen_casefold(:text)) > 0)";

const PAGE_SUFFIX: &str = ", matched AS MATERIALIZED (
 SELECT c.id, CASE WHEN instr(kaizen_casefold(c.prompt), kaizen_casefold(:text)) > 0
  THEN 0 ELSE 1 END match_tier
 FROM candidates c WHERE ";

const SUMMARY_SUFFIX: &str = "
 ORDER BY match_tier, c.started_at_ms DESC, c.id ASC LIMIT :limit OFFSET :offset
), rollup AS (
 SELECT e.session_id, MAX(e.ts_ms) last_event_ms, COUNT(*) event_count,
  SUM(e.kind = 'Error') error_count, SUM(e.kind = 'ToolCall') tool_call_count,
  COALESCE(SUM(e.cost_usd_e6), 0) cost_usd_e6,
  COALESCE(SUM(e.tokens_in), 0) tokens_in, COALESCE(SUM(e.tokens_out), 0) tokens_out,
  COALESCE(SUM(e.reasoning_tokens), 0) reasoning_tokens,
  COALESCE(SUM(e.cache_read_tokens), 0) cache_read_tokens,
  COALESCE(SUM(e.cache_creation_tokens), 0) cache_creation_tokens
 FROM events e JOIN matched m ON m.id = e.session_id GROUP BY e.session_id
)
SELECT s.id, s.agent, s.model, s.workspace, s.started_at_ms, s.ended_at_ms,
 s.status, s.trace_path, s.start_commit, s.end_commit, s.branch, s.dirty_start, s.dirty_end,
 s.repo_binding_source, s.prompt_fingerprint, s.parent_session_id, s.agent_version, s.os, s.arch,
 s.repo_file_count, s.repo_total_loc, a.last_event_ms, COALESCE(a.event_count, 0),
 COALESCE(a.error_count, 0), COALESCE(a.tool_call_count, 0), COALESCE(a.cost_usd_e6, 0),
 COALESCE(a.tokens_in, 0), COALESCE(a.tokens_out, 0), COALESCE(a.reasoning_tokens, 0),
 COALESCE(a.cache_read_tokens, 0), COALESCE(a.cache_creation_tokens, 0)
FROM matched m JOIN sessions s ON s.id = m.id LEFT JOIN rollup a ON a.session_id = s.id
ORDER BY m.match_tier, s.started_at_ms DESC, s.id ASC";

pub(super) fn count(with_status: bool) -> String {
    let candidates = candidates(with_status);
    let matches = matches(with_status);
    format!("{candidates} SELECT COUNT(*) FROM candidates c WHERE {matches}")
}

pub(super) fn page(with_status: bool) -> String {
    let candidates = candidates(with_status);
    let matches = matches(with_status);
    format!("{candidates}{PAGE_SUFFIX}{matches}{SUMMARY_SUFFIX}")
}

fn candidates(with_status: bool) -> &'static str {
    if with_status {
        STATUS_CANDIDATES
    } else {
        ORDINARY_CANDIDATES
    }
}

fn matches(with_status: bool) -> String {
    let status = if with_status {
        "\n OR instr(kaizen_casefold(c.derived_status), kaizen_casefold(:text)) > 0"
    } else {
        ""
    };
    format!("{MATCH_PREFIX}{status}{TOOL_MATCHES})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use rusqlite::named_params;

    #[test]
    fn ordinary_sql_omits_status_rollup() {
        for sql in [count(false), page(false)] {
            assert!(!sql.contains("status_rollup"), "{sql}");
        }
    }

    #[test]
    fn visible_status_sql_includes_status_rollup() {
        for sql in [count(true), page(true)] {
            assert!(sql.contains("status_rollup"), "{sql}");
        }
    }

    #[test]
    fn ordinary_page_plan_omits_status_rollup() -> anyhow::Result<()> {
        let temp = tempfile::tempdir()?;
        let store = Store::open(&temp.path().join("kaizen.db"))?;
        let plan = explain(&store, &page(false))?;
        assert!(plan.iter().all(|step| !step.contains("status_rollup")));
        Ok(())
    }

    fn explain(store: &Store, sql: &str) -> anyhow::Result<Vec<String>> {
        let sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = store.conn().prepare(&sql)?;
        let values = named_params! {":workspace": "/ws", ":text": "needle",
        ":limit": 30_i64, ":offset": 0_i64};
        let rows = statement.query_map(values, |row| row.get(3))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
