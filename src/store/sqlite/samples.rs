use super::*;

impl Store {
    pub fn append_session_sample(
        &self,
        session_id: &str,
        ts_ms: u64,
        pid: u32,
        cpu_percent: Option<f64>,
        rss_bytes: Option<u64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO session_samples (session_id, ts_ms, pid, cpu_percent, rss_bytes)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                ts_ms as i64,
                pid as i64,
                cpu_percent,
                rss_bytes.map(|b| b as i64)
            ],
        )?;
        Ok(())
    }

    /// Per-session maxima for retro heuristics.
    pub fn list_session_sample_aggs_in_window(
        &self,
        workspace: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<SessionSampleAgg>> {
        let mut stmt = self.conn.prepare(
            "SELECT ss.session_id, COUNT(*) AS n,
                    MAX(ss.cpu_percent), MAX(ss.rss_bytes)
             FROM session_samples ss
             JOIN sessions s ON s.id = ss.session_id
             WHERE s.workspace = ?1 AND s.started_at_ms >= ?2 AND s.started_at_ms <= ?3
             GROUP BY ss.session_id",
        )?;
        let rows = stmt.query_map(params![workspace, start_ms as i64, end_ms as i64], |r| {
            let sid: String = r.get(0)?;
            let n: i64 = r.get(1)?;
            let max_cpu: Option<f64> = r.get(2)?;
            let max_rss: Option<i64> = r.get(3)?;
            Ok(SessionSampleAgg {
                session_id: sid,
                sample_count: n as u64,
                max_cpu_percent: max_cpu.unwrap_or(0.0),
                max_rss_bytes: max_rss.map(|x| x as u64).unwrap_or(0),
            })
        })?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }
}
