use super::rows::*;
use super::*;

impl Store {
    pub fn upsert_session_outcome(&self, row: &SessionOutcomeRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_outcomes (
                session_id, test_passed, test_failed, test_skipped, build_ok, lint_errors,
                revert_lines_14d, pr_open, ci_ok, measured_at_ms, measure_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(session_id) DO UPDATE SET
                test_passed=excluded.test_passed,
                test_failed=excluded.test_failed,
                test_skipped=excluded.test_skipped,
                build_ok=excluded.build_ok,
                lint_errors=excluded.lint_errors,
                revert_lines_14d=excluded.revert_lines_14d,
                pr_open=excluded.pr_open,
                ci_ok=excluded.ci_ok,
                measured_at_ms=excluded.measured_at_ms,
                measure_error=excluded.measure_error",
            params![
                row.session_id,
                row.test_passed,
                row.test_failed,
                row.test_skipped,
                row.build_ok.map(bool_to_i64),
                row.lint_errors,
                row.revert_lines_14d,
                row.pr_open,
                row.ci_ok.map(bool_to_i64),
                row.measured_at_ms as i64,
                row.measure_error.as_deref(),
            ],
        )?;
        Ok(())
    }

    pub fn get_session_outcome(&self, session_id: &str) -> Result<Option<SessionOutcomeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, test_passed, test_failed, test_skipped, build_ok, lint_errors,
                    revert_lines_14d, pr_open, ci_ok, measured_at_ms, measure_error
             FROM session_outcomes WHERE session_id = ?1",
        )?;
        let row = stmt
            .query_row(params![session_id], outcome_row)
            .optional()?;
        Ok(row)
    }

    /// Outcomes for sessions in `workspace` whose `started_at` falls in the window.
    pub fn list_session_outcomes_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<SessionOutcomeRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT o.session_id, o.test_passed, o.test_failed, o.test_skipped, o.build_ok, o.lint_errors,
                    o.revert_lines_14d, o.pr_open, o.ci_ok, o.measured_at_ms, o.measure_error
             FROM session_outcomes o
             JOIN sessions s ON s.id = o.session_id
             WHERE s.workspace = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3
             ORDER BY o.measured_at_ms ASC",
        )?;
        let rows = stmt.query_map(
            params![workspace, start_ms as i64, end_ms as i64],
            outcome_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }
}
