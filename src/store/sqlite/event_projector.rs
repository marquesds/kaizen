use super::events::*;
use super::*;

impl Store {
    pub(super) fn sync_projector_session(
        &self,
        session_id: &str,
        last_seq: Option<u64>,
    ) -> Result<()> {
        if self.projector.borrow().last_seq(session_id) == last_seq {
            return Ok(());
        }
        self.replay_projector_session(session_id)
    }

    pub fn flush_projector_session(&self, session_id: &str, now_ms: u64) -> Result<()> {
        if projector_legacy_mode() {
            rebuild_tool_spans_for_session(&self.conn, session_id)?;
            self.invalidate_span_tree_cache();
            return Ok(());
        }
        let deltas = self
            .projector
            .borrow_mut()
            .flush_session(session_id, now_ms);
        if self.apply_projector_events(&deltas)? {
            self.invalidate_span_tree_cache();
        }
        Ok(())
    }

    pub(super) fn replay_projector_session(&self, session_id: &str) -> Result<()> {
        clear_session_spans(&self.conn, session_id)?;
        self.projector.borrow_mut().reset_session(session_id);
        let events = self.list_events_for_session(session_id)?;
        let mut changed = false;
        for event in &events {
            let deltas = self.projector.borrow_mut().apply(event);
            changed |= self.apply_projector_events(&deltas)?;
        }
        if self
            .get_session(session_id)?
            .is_some_and(|session| session.status == SessionStatus::Done)
        {
            let now_ms = events.last().map(|event| event.ts_ms).unwrap_or(0);
            let deltas = self
                .projector
                .borrow_mut()
                .flush_session(session_id, now_ms);
            changed |= self.apply_projector_events(&deltas)?;
        }
        if changed {
            self.invalidate_span_tree_cache();
        }
        Ok(())
    }

    pub(super) fn apply_projector_events(&self, deltas: &[ProjectorEvent]) -> Result<bool> {
        let mut changed = false;
        for delta in deltas {
            match delta {
                ProjectorEvent::SpanClosed(span, sample) => {
                    upsert_tool_span_record(&self.conn, span)?;
                    tracing::debug!(
                        session_id = %sample.session_id,
                        span_id = %sample.span_id,
                        tool = ?sample.tool,
                        lead_time_ms = ?sample.lead_time_ms,
                        tokens_in = ?sample.tokens_in,
                        tokens_out = ?sample.tokens_out,
                        reasoning_tokens = ?sample.reasoning_tokens,
                        cost_usd_e6 = ?sample.cost_usd_e6,
                        paths = ?sample.paths,
                        "tool span closed"
                    );
                    changed = true;
                }
                ProjectorEvent::FileTouched { session, path } => {
                    self.conn.execute(
                        "INSERT OR IGNORE INTO files_touched (session_id, path) VALUES (?1, ?2)",
                        params![session, path],
                    )?;
                    changed = true;
                }
                ProjectorEvent::SkillUsed { session, skill } => {
                    self.conn.execute(
                        "INSERT OR IGNORE INTO skills_used (session_id, skill) VALUES (?1, ?2)",
                        params![session, skill],
                    )?;
                    changed = true;
                }
                ProjectorEvent::RuleUsed { session, rule } => {
                    self.conn.execute(
                        "INSERT OR IGNORE INTO rules_used (session_id, rule) VALUES (?1, ?2)",
                        params![session, rule],
                    )?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}
