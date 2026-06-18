// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) const SESSION_SELECT: &str =
    "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms,
    status, trace_path, start_commit, end_commit, branch, dirty_start, dirty_end,
    repo_binding_source, prompt_fingerprint, parent_session_id, agent_version, os, arch,
    repo_file_count, repo_total_loc FROM sessions";
pub(crate) const PAIN_HOTSPOTS_SQL: &str = "
    SELECT f.path,
           COUNT(s.id) * f.complexity_total AS value,
           f.complexity_total,
           f.churn_30d
    FROM file_facts f
    LEFT JOIN tool_span_paths tsp ON tsp.path = f.path
    LEFT JOIN tool_spans ts ON ts.span_id = tsp.span_id
       AND ((ts.started_at_ms >= ?3 AND ts.started_at_ms <= ?4)
         OR (ts.started_at_ms IS NULL AND ts.ended_at_ms >= ?3 AND ts.ended_at_ms <= ?4))
    LEFT JOIN sessions s ON s.id = ts.session_id AND s.workspace = ?2
    WHERE f.snapshot_id = ?1
    GROUP BY f.path, f.complexity_total, f.churn_30d
    ORDER BY value DESC, f.path ASC
    LIMIT 10";
pub(crate) const TOOL_RANK_ROWS_SQL: &str = "
    WITH scoped AS (
      SELECT COALESCE(ts.tool, 'unknown') AS tool,
             ts.lead_time_ms,
             COALESCE(ts.tokens_in, 0) + COALESCE(ts.tokens_out, 0)
                 + COALESCE(ts.reasoning_tokens, 0) AS total_tokens,
             COALESCE(ts.reasoning_tokens, 0) AS reasoning_tokens
      FROM tool_spans ts
      JOIN sessions s ON s.id = ts.session_id
      WHERE s.workspace = ?1
        AND ((ts.started_at_ms >= ?2 AND ts.started_at_ms <= ?3)
          OR (ts.started_at_ms IS NULL AND ts.ended_at_ms >= ?2 AND ts.ended_at_ms <= ?3))
    ),
    agg AS (
      SELECT tool, COUNT(*) AS calls, SUM(total_tokens) AS total_tokens,
             SUM(reasoning_tokens) AS total_reasoning_tokens
      FROM scoped GROUP BY tool
    ),
    lat AS (
      SELECT tool, lead_time_ms,
             ROW_NUMBER() OVER (PARTITION BY tool ORDER BY lead_time_ms) AS rn,
             COUNT(*) OVER (PARTITION BY tool) AS n
      FROM scoped WHERE lead_time_ms IS NOT NULL
    ),
    pct AS (
      SELECT tool,
             MAX(CASE WHEN rn = CAST(((n - 1) * 50) / 100 AS INTEGER) + 1 THEN lead_time_ms END) AS p50_ms,
             MAX(CASE WHEN rn = CAST(((n - 1) * 95) / 100 AS INTEGER) + 1 THEN lead_time_ms END) AS p95_ms
      FROM lat GROUP BY tool
    )
    SELECT agg.tool, agg.calls, pct.p50_ms, pct.p95_ms,
           agg.total_tokens, agg.total_reasoning_tokens
    FROM agg LEFT JOIN pct ON pct.tool = agg.tool";
