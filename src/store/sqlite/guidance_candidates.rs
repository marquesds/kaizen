use super::*;

impl Store {
    pub fn upsert_guidance_candidate(&self, c: &crate::guidance::GuidanceCandidate) -> Result<()> {
        let action_json = serde_json::to_string(&c.action)?;
        let evidence_json = serde_json::to_string(&c.evidence)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO guidance_candidates
             (id, artifact_kind, artifact_id, action_json, status, rationale,
              evidence_json, created_at_ms, applied_at_ms, treatment_fingerprint,
              experiment_id, backup_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                c.id,
                c.artifact.kind.as_str(),
                c.artifact.slug,
                action_json,
                c.status.as_str(),
                c.rationale,
                evidence_json,
                c.created_at_ms as i64,
                c.applied_at_ms.map(|v| v as i64),
                c.treatment_fingerprint.as_deref(),
                c.experiment_id.as_deref(),
                c.backup_path.as_deref(),
            ],
        )?;
        Ok(())
    }

    pub fn list_guidance_candidates(&self) -> Result<Vec<crate::guidance::GuidanceCandidate>> {
        let mut stmt = self.conn.prepare(GUIDANCE_CANDIDATE_SELECT)?;
        let rows = stmt.query_map([], guidance_candidate_row)?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn get_guidance_candidate(
        &self,
        id: &str,
    ) -> Result<Option<crate::guidance::GuidanceCandidate>> {
        self.conn
            .query_row(
                &format!("{GUIDANCE_CANDIDATE_SELECT} WHERE id = ?1"),
                params![id],
                guidance_candidate_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn rejected_guidance_candidates(
        &self,
        artifact: &crate::guidance::ArtifactRef,
        limit: usize,
    ) -> Result<Vec<crate::guidance::GuidanceCandidate>> {
        let sql = format!(
            "{GUIDANCE_CANDIDATE_SELECT} WHERE artifact_kind = ?1 AND artifact_id = ?2
             AND status = 'rejected' ORDER BY created_at_ms DESC LIMIT ?3"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![artifact.kind.as_str(), artifact.slug.as_str(), limit as i64],
            guidance_candidate_row,
        )?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn set_guidance_candidate_status(
        &self,
        id: &str,
        status: crate::guidance::CandidateStatus,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE guidance_candidates SET status = ?2 WHERE id = ?1",
            params![id, status.as_str()],
        )?;
        Ok(())
    }
}

const GUIDANCE_CANDIDATE_SELECT: &str = "SELECT id, artifact_kind, artifact_id, action_json,
    status, rationale, evidence_json, created_at_ms, applied_at_ms,
    treatment_fingerprint, experiment_id, backup_path FROM guidance_candidates";

fn guidance_candidate_row(
    r: &rusqlite::Row<'_>,
) -> rusqlite::Result<crate::guidance::GuidanceCandidate> {
    let kind: String = r.get(1)?;
    let status: String = r.get(4)?;
    Ok(crate::guidance::GuidanceCandidate {
        id: r.get(0)?,
        artifact: crate::guidance::ArtifactRef {
            kind: crate::guidance::ArtifactKind::parse(&kind)
                .unwrap_or(crate::guidance::ArtifactKind::Skill),
            slug: r.get(2)?,
        },
        action: serde_json::from_str(&r.get::<_, String>(3)?)
            .unwrap_or(crate::guidance::CandidateAction::ReviewOnly),
        status: crate::guidance::CandidateStatus::parse(&status)
            .unwrap_or(crate::guidance::CandidateStatus::Proposed),
        rationale: r.get(5)?,
        evidence: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
        created_at_ms: r.get::<_, i64>(7)? as u64,
        applied_at_ms: r.get::<_, Option<i64>>(8)?.map(|v| v as u64),
        treatment_fingerprint: r.get(9)?,
        experiment_id: r.get(10)?,
        backup_path: r.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guidance::{ArtifactKind, ArtifactRef, CandidateAction, CandidateStatus};

    #[test]
    fn candidate_round_trip_and_status() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = Store::open(&dir.path().join("k.db"))?;
        let c = candidate();
        store.upsert_guidance_candidate(&c)?;
        store.set_guidance_candidate_status("c1", CandidateStatus::Rejected)?;
        let got = store.get_guidance_candidate("c1")?.unwrap();
        assert_eq!(got.status, CandidateStatus::Rejected);
        Ok(())
    }

    #[test]
    fn rejected_candidates_filter_by_artifact() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = Store::open(&dir.path().join("k.db"))?;
        store.upsert_guidance_candidate(&candidate())?;
        store.set_guidance_candidate_status("c1", CandidateStatus::Rejected)?;
        let got = store.rejected_guidance_candidates(&candidate().artifact, 10)?;
        assert_eq!(got.len(), 1);
        Ok(())
    }

    fn candidate() -> crate::guidance::GuidanceCandidate {
        crate::guidance::GuidanceCandidate {
            id: "c1".into(),
            artifact: ArtifactRef {
                kind: ArtifactKind::Skill,
                slug: "tdd".into(),
            },
            action: CandidateAction::ReviewOnly,
            status: CandidateStatus::Proposed,
            rationale: "inspect".into(),
            evidence: vec!["e".into()],
            created_at_ms: 1,
            applied_at_ms: None,
            treatment_fingerprint: None,
            experiment_id: None,
            backup_path: None,
        }
    }
}
