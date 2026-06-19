use super::rows::*;
use super::*;

impl Store {
    pub fn upsert_session(&self, s: &SessionRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (
                id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                prompt_fingerprint, parent_session_id, agent_version, os, arch,
                repo_file_count, repo_total_loc
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21)
             ON CONFLICT(id) DO UPDATE SET
               agent=excluded.agent, model=excluded.model, workspace=excluded.workspace,
               started_at_ms=excluded.started_at_ms, ended_at_ms=excluded.ended_at_ms,
               status=excluded.status, trace_path=excluded.trace_path,
               start_commit=excluded.start_commit, end_commit=excluded.end_commit,
               branch=excluded.branch, dirty_start=excluded.dirty_start,
               dirty_end=excluded.dirty_end, repo_binding_source=excluded.repo_binding_source,
               prompt_fingerprint=excluded.prompt_fingerprint,
               parent_session_id=excluded.parent_session_id,
               agent_version=excluded.agent_version, os=excluded.os, arch=excluded.arch,
               repo_file_count=excluded.repo_file_count, repo_total_loc=excluded.repo_total_loc",
            params![
                s.id,
                s.agent,
                s.model,
                s.workspace,
                s.started_at_ms as i64,
                s.ended_at_ms.map(|v| v as i64),
                format!("{:?}", s.status),
                s.trace_path,
                s.start_commit,
                s.end_commit,
                s.branch,
                s.dirty_start.map(bool_to_i64),
                s.dirty_end.map(bool_to_i64),
                s.repo_binding_source.clone().unwrap_or_default(),
                s.prompt_fingerprint.as_deref(),
                s.parent_session_id.as_deref(),
                s.agent_version.as_deref(),
                s.os.as_deref(),
                s.arch.as_deref(),
                s.repo_file_count.map(|v| v as i64),
                s.repo_total_loc.map(|v| v as i64),
            ],
        )?;
        self.conn.execute(
            "INSERT INTO session_repo_binding (
                session_id, start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id) DO UPDATE SET
                start_commit=excluded.start_commit,
                end_commit=excluded.end_commit,
                branch=excluded.branch,
                dirty_start=excluded.dirty_start,
                dirty_end=excluded.dirty_end,
                repo_binding_source=excluded.repo_binding_source",
            params![
                s.id,
                s.start_commit,
                s.end_commit,
                s.branch,
                s.dirty_start.map(bool_to_i64),
                s.dirty_end.map(bool_to_i64),
                s.repo_binding_source.clone().unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    /// Insert a minimal session row if none exists. Used by hook ingestion when
    /// the first observed event is not `SessionStart` (hooks installed mid-session).
    pub fn ensure_session_stub(
        &self,
        id: &str,
        agent: &str,
        workspace: &str,
        started_at_ms: u64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO sessions (
                id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path,
                start_commit, end_commit, branch, dirty_start, dirty_end, repo_binding_source,
                prompt_fingerprint, parent_session_id, agent_version, os, arch, repo_file_count, repo_total_loc
             ) VALUES (?1, ?2, NULL, ?3, ?4, NULL, 'Running', '', NULL, NULL, NULL, NULL, NULL, '',
                NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
            params![id, agent, workspace, started_at_ms as i64],
        )?;
        Ok(())
    }

    pub fn enrich_session_identity(
        &self,
        id: &str,
        agent: Option<&str>,
        model: Option<&str>,
        trace_path: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET agent = COALESCE(?2, agent), model = COALESCE(?3, model),
             trace_path = CASE WHEN COALESCE(?4, '') = '' THEN trace_path ELSE ?4 END WHERE id = ?1",
            params![id, agent, model, trace_path],
        )?;
        Ok(())
    }

    /// Update only status for existing session.
    pub fn update_session_status(&self, id: &str, status: SessionStatus) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET status = ?1 WHERE id = ?2",
            params![format!("{:?}", status), id],
        )?;
        Ok(())
    }
}
