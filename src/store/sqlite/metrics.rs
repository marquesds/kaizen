use super::rows::*;
use super::*;
impl Store {
    pub fn latest_repo_snapshot(&self, workspace: &str) -> Result<Option<RepoSnapshotRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, workspace, head_commit, dirty_fingerprint, analyzer_version,
                    indexed_at_ms, dirty, graph_path
             FROM repo_snapshots WHERE workspace = ?1
             ORDER BY indexed_at_ms DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![workspace], |row| {
            Ok(RepoSnapshotRecord {
                id: row.get(0)?,
                workspace: row.get(1)?,
                head_commit: row.get(2)?,
                dirty_fingerprint: row.get(3)?,
                analyzer_version: row.get(4)?,
                indexed_at_ms: row.get::<_, i64>(5)? as u64,
                dirty: row.get::<_, i64>(6)? != 0,
                graph_path: row.get(7)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn save_repo_snapshot(
        &self,
        snapshot: &RepoSnapshotRecord,
        facts: &[FileFact],
        edges: &[RepoEdge],
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO repo_snapshots (
                id, workspace, head_commit, dirty_fingerprint, analyzer_version,
                indexed_at_ms, dirty, graph_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                workspace=excluded.workspace,
                head_commit=excluded.head_commit,
                dirty_fingerprint=excluded.dirty_fingerprint,
                analyzer_version=excluded.analyzer_version,
                indexed_at_ms=excluded.indexed_at_ms,
                dirty=excluded.dirty,
                graph_path=excluded.graph_path",
            params![
                snapshot.id,
                snapshot.workspace,
                snapshot.head_commit,
                snapshot.dirty_fingerprint,
                snapshot.analyzer_version,
                snapshot.indexed_at_ms as i64,
                bool_to_i64(snapshot.dirty),
                snapshot.graph_path,
            ],
        )?;
        self.conn.execute(
            "DELETE FROM file_facts WHERE snapshot_id = ?1",
            params![snapshot.id],
        )?;
        self.conn.execute(
            "DELETE FROM repo_edges WHERE snapshot_id = ?1",
            params![snapshot.id],
        )?;
        for fact in facts {
            self.conn.execute(
                "INSERT INTO file_facts (
                    snapshot_id, path, language, bytes, loc, sloc, complexity_total,
                    max_fn_complexity, symbol_count, import_count, fan_in, fan_out,
                    churn_30d, churn_90d, authors_90d, last_changed_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    fact.snapshot_id,
                    fact.path,
                    fact.language,
                    fact.bytes as i64,
                    fact.loc as i64,
                    fact.sloc as i64,
                    fact.complexity_total as i64,
                    fact.max_fn_complexity as i64,
                    fact.symbol_count as i64,
                    fact.import_count as i64,
                    fact.fan_in as i64,
                    fact.fan_out as i64,
                    fact.churn_30d as i64,
                    fact.churn_90d as i64,
                    fact.authors_90d as i64,
                    fact.last_changed_ms.map(|v| v as i64),
                ],
            )?;
        }
        for edge in edges {
            self.conn.execute(
                "INSERT INTO repo_edges (snapshot_id, from_id, to_id, kind, weight)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(snapshot_id, from_id, to_id, kind)
                 DO UPDATE SET weight = weight + excluded.weight",
                params![
                    snapshot.id,
                    edge.from_path,
                    edge.to_path,
                    edge.kind,
                    edge.weight as i64,
                ],
            )?;
        }
        Ok(())
    }

    pub fn file_facts_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<FileFact>> {
        let mut stmt = self.conn.prepare(
            "SELECT snapshot_id, path, language, bytes, loc, sloc, complexity_total,
                    max_fn_complexity, symbol_count, import_count, fan_in, fan_out,
                    churn_30d, churn_90d, authors_90d, last_changed_ms
             FROM file_facts WHERE snapshot_id = ?1 ORDER BY path ASC",
        )?;
        let rows = stmt.query_map(params![snapshot_id], |row| {
            Ok(FileFact {
                snapshot_id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
                bytes: row.get::<_, i64>(3)? as u64,
                loc: row.get::<_, i64>(4)? as u32,
                sloc: row.get::<_, i64>(5)? as u32,
                complexity_total: row.get::<_, i64>(6)? as u32,
                max_fn_complexity: row.get::<_, i64>(7)? as u32,
                symbol_count: row.get::<_, i64>(8)? as u32,
                import_count: row.get::<_, i64>(9)? as u32,
                fan_in: row.get::<_, i64>(10)? as u32,
                fan_out: row.get::<_, i64>(11)? as u32,
                churn_30d: row.get::<_, i64>(12)? as u32,
                churn_90d: row.get::<_, i64>(13)? as u32,
                authors_90d: row.get::<_, i64>(14)? as u32,
                last_changed_ms: row.get::<_, Option<i64>>(15)?.map(|v| v as u64),
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }
    pub fn repo_edges_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RepoEdge>> {
        let mut stmt = self.conn.prepare(
            "SELECT from_id, to_id, kind, weight
             FROM repo_edges WHERE snapshot_id = ?1
             ORDER BY kind, from_id, to_id",
        )?;
        let rows = stmt.query_map(params![snapshot_id], |row| {
            Ok(RepoEdge {
                from_path: row.get(0)?,
                to_path: row.get(1)?,
                kind: row.get(2)?,
                weight: row.get::<_, i64>(3)? as u32,
            })
        })?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }
    pub fn hottest_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "churn_30d * complexity_total")
    }

    pub fn most_changed_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "churn_30d")
    }

    pub fn most_complex_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "complexity_total")
    }

    pub fn highest_risk_files_for_snapshot(&self, snapshot_id: &str) -> Result<Vec<RankedFile>> {
        self.ranked_files_for_snapshot(snapshot_id, "churn_30d * authors_90d * complexity_total")
    }

    pub(super) fn ranked_files_for_snapshot(
        &self,
        snapshot_id: &str,
        value_sql: &str,
    ) -> Result<Vec<RankedFile>> {
        let sql = format!(
            "SELECT path, {value_sql}, complexity_total, churn_30d
             FROM file_facts WHERE snapshot_id = ?1
             ORDER BY {value_sql} DESC, path ASC LIMIT 10"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![snapshot_id], ranked_file_row)?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn pain_hotspots_for_snapshot(
        &self,
        snapshot_id: &str,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<RankedFile>> {
        let mut stmt = self.conn.prepare(PAIN_HOTSPOTS_SQL)?;
        let rows = stmt.query_map(
            params![snapshot_id, workspace, start_ms as i64, end_ms as i64],
            ranked_file_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }
}
