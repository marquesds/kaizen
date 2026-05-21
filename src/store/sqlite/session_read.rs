use super::rows::*;
use super::*;

impl Store {
    pub fn list_sessions(&self, workspace: &str) -> Result<Vec<SessionRecord>> {
        Ok(self
            .list_sessions_page(workspace, 0, i64::MAX as usize, SessionFilter::default())?
            .rows)
    }

    pub fn list_sessions_page(
        &self,
        workspace: &str,
        offset: usize,
        limit: usize,
        filter: SessionFilter,
    ) -> Result<SessionPage> {
        let (where_sql, args) = session_filter_sql(workspace, &filter);
        let total = self.query_session_page_count(&where_sql, &args)?;
        let rows = self.query_session_page_rows(&where_sql, &args, offset, limit)?;
        let next = offset.saturating_add(rows.len());
        Ok(SessionPage {
            rows,
            total,
            next_offset: (next < total).then_some(next),
        })
    }

    pub(super) fn query_session_page_count(
        &self,
        where_sql: &str,
        args: &[Value],
    ) -> Result<usize> {
        let sql = format!("SELECT COUNT(*) FROM sessions {where_sql}");
        let total: i64 = self
            .conn
            .query_row(&sql, params_from_iter(args.iter()), |r| r.get(0))?;
        Ok(total as usize)
    }

    pub(super) fn query_session_page_rows(
        &self,
        where_sql: &str,
        args: &[Value],
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SessionRecord>> {
        let sql = format!(
            "{SESSION_SELECT} {where_sql} ORDER BY started_at_ms DESC, id ASC LIMIT ? OFFSET ?"
        );
        let mut values = args.to_vec();
        values.push(Value::Integer(limit.min(i64::MAX as usize) as i64));
        values.push(Value::Integer(offset.min(i64::MAX as usize) as i64));
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(values.iter()), session_row)?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn list_sessions_started_after(
        &self,
        workspace: &str,
        after_started_at_ms: u64,
    ) -> Result<Vec<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                    start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                    prompt_fingerprint, parent_session_id, agent_version, os, arch,
                    repo_file_count, repo_total_loc
             FROM sessions
             WHERE workspace = ?1 AND started_at_ms > ?2
             ORDER BY started_at_ms DESC, id ASC",
        )?;
        let rows = stmt.query_map(params![workspace, after_started_at_ms as i64], session_row)?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn session_statuses(&self, ids: &[String]) -> Result<Vec<SessionStatusRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql =
            format!("SELECT id, status, ended_at_ms FROM sessions WHERE id IN ({placeholders})");
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |r| {
            let status: String = r.get(1)?;
            Ok(SessionStatusRow {
                id: r.get(0)?,
                status: status_from_str(&status),
                ended_at_ms: r.get::<_, Option<i64>>(2)?.map(|v| v as u64),
            })
        })?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub(super) fn running_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM sessions WHERE status != 'Done' ORDER BY started_at_ms ASC")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn get_session(&self, id: &str) -> Result<Option<SessionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                    start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                    prompt_fingerprint, parent_session_id, agent_version, os, arch,
                    repo_file_count, repo_total_loc
             FROM sessions WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, Option<i64>>(12)?,
                row.get::<_, String>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, Option<String>>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<i64>>(19)?,
                row.get::<_, Option<i64>>(20)?,
            ))
        })?;

        if let Some(row) = rows.next() {
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
            Ok(Some(SessionRecord {
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
                repo_binding_source: empty_to_none(source),
                prompt_fingerprint,
                parent_session_id,
                agent_version,
                os,
                arch,
                repo_file_count: repo_file_count.map(|v| v as u32),
                repo_total_loc: repo_total_loc.map(|v| v as u64),
            }))
        } else {
            Ok(None)
        }
    }
}
