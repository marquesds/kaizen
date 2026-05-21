use super::rows::*;
use super::*;

impl Store {
    pub fn upsert_eval(&self, eval: &crate::eval::types::EvalRow) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO session_evals
             (id, session_id, judge_model, rubric_id, score, rationale, flagged, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                eval.id,
                eval.session_id,
                eval.judge_model,
                eval.rubric_id,
                eval.score,
                eval.rationale,
                eval.flagged as i64,
                eval.created_at_ms as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_evals_in_window(
        &self,
        start_ms: u64,
        end_ms: u64,
    ) -> rusqlite::Result<Vec<crate::eval::types::EvalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, judge_model, rubric_id, score,
                    rationale, flagged, created_at_ms
             FROM session_evals
             WHERE created_at_ms >= ?1 AND created_at_ms < ?2
             ORDER BY created_at_ms ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![start_ms as i64, end_ms as i64], |r| {
            Ok(crate::eval::types::EvalRow {
                id: r.get(0)?,
                session_id: r.get(1)?,
                judge_model: r.get(2)?,
                rubric_id: r.get(3)?,
                score: r.get(4)?,
                rationale: r.get(5)?,
                flagged: r.get::<_, i64>(6)? != 0,
                created_at_ms: r.get::<_, i64>(7)? as u64,
            })
        })?;
        rows.collect()
    }

    pub fn list_evals_for_session(
        &self,
        session_id: &str,
    ) -> rusqlite::Result<Vec<crate::eval::types::EvalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, judge_model, rubric_id, score,
                    rationale, flagged, created_at_ms
             FROM session_evals
             WHERE session_id = ?1
             ORDER BY created_at_ms DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![session_id], |r| {
            Ok(crate::eval::types::EvalRow {
                id: r.get(0)?,
                session_id: r.get(1)?,
                judge_model: r.get(2)?,
                rubric_id: r.get(3)?,
                score: r.get(4)?,
                rationale: r.get(5)?,
                flagged: r.get::<_, i64>(6)? != 0,
                created_at_ms: r.get::<_, i64>(7)? as u64,
            })
        })?;
        rows.collect()
    }

    pub fn list_sessions_for_eval(
        &self,
        since_ms: u64,
        min_cost_usd: f64,
    ) -> Result<Vec<crate::core::event::SessionRecord>> {
        let min_cost_e6 = (min_cost_usd * 1_000_000.0) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.agent, s.model, s.workspace, s.started_at_ms, s.ended_at_ms,
                    s.status, s.trace_path, s.start_commit, s.end_commit, s.branch,
                    s.dirty_start, s.dirty_end, s.repo_binding_source, s.prompt_fingerprint,
                    s.parent_session_id, s.agent_version, s.os, s.arch, s.repo_file_count, s.repo_total_loc
             FROM sessions s
             WHERE s.started_at_ms >= ?1
               AND COALESCE((SELECT SUM(e.cost_usd_e6) FROM events e WHERE e.session_id = s.id), 0) >= ?2
               AND NOT EXISTS (SELECT 1 FROM session_evals ev WHERE ev.session_id = s.id)
             ORDER BY s.started_at_ms DESC",
        )?;
        let rows = stmt.query_map(params![since_ms as i64, min_cost_e6], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, Option<i64>>(11)?,
                r.get::<_, Option<i64>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<String>>(16)?,
                r.get::<_, Option<String>>(17)?,
                r.get::<_, Option<String>>(18)?,
                r.get::<_, Option<i64>>(19)?,
                r.get::<_, Option<i64>>(20)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                id,
                agent,
                model,
                workspace,
                started,
                ended,
                status_str,
                trace,
                start_commit,
                end_commit,
                branch,
                dirty_start,
                dirty_end,
                source,
                prompt_fingerprint,
                parent_session_id,
                agent_version,
                os,
                arch,
                repo_file_count,
                repo_total_loc,
            ) = row?;
            out.push(crate::core::event::SessionRecord {
                id,
                agent,
                model,
                workspace,
                started_at_ms: started as u64,
                ended_at_ms: ended.map(|v| v as u64),
                status: status_from_str(&status_str),
                trace_path: trace,
                start_commit,
                end_commit,
                branch,
                dirty_start: dirty_start.map(i64_to_bool),
                dirty_end: dirty_end.map(i64_to_bool),
                repo_binding_source: source.and_then(|s| if s.is_empty() { None } else { Some(s) }),
                prompt_fingerprint,
                parent_session_id,
                agent_version,
                os,
                arch,
                repo_file_count: repo_file_count.map(|v| v as u32),
                repo_total_loc: repo_total_loc.map(|v| v as u64),
            });
        }
        Ok(out)
    }
}
