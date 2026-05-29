use super::events::*;
use super::rows::*;
use super::*;

impl Store {
    /// Next `seq` for a new event in this session (0 when there are no events yet).
    pub fn next_event_seq(&self, session_id: &str) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(seq) + 1, 0) FROM events WHERE session_id = ?1",
            [session_id],
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }

    pub fn append_event(&self, e: &Event) -> Result<()> {
        self.append_event_with_sync(e, None)
    }

    /// Append event; when `ctx` is set and sync is configured, enqueue one redacted outbox row.
    pub fn append_event_with_sync(&self, e: &Event, ctx: Option<&SyncIngestContext>) -> Result<()> {
        let last_before = if projector_legacy_mode() {
            None
        } else {
            self.last_event_seq_for_session(&e.session_id)?
        };
        let payload = serde_json::to_string(&e.payload)?;
        self.conn.execute(
            "INSERT INTO events (
                session_id, seq, ts_ms, ts_exact, kind, source, tool, tool_call_id,
                tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload,
                stop_reason, latency_ms, ttft_ms, retry_count,
                context_used_tokens, context_max_tokens,
                cache_creation_tokens, cache_read_tokens, system_prompt_tokens
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
             )
             ON CONFLICT(session_id, seq) DO UPDATE SET
                ts_ms = excluded.ts_ms,
                ts_exact = excluded.ts_exact,
                kind = excluded.kind,
                source = excluded.source,
                tool = excluded.tool,
                tool_call_id = excluded.tool_call_id,
                tokens_in = excluded.tokens_in,
                tokens_out = excluded.tokens_out,
                reasoning_tokens = excluded.reasoning_tokens,
                cost_usd_e6 = excluded.cost_usd_e6,
                payload = excluded.payload,
                stop_reason = excluded.stop_reason,
                latency_ms = excluded.latency_ms,
                ttft_ms = excluded.ttft_ms,
                retry_count = excluded.retry_count,
                context_used_tokens = excluded.context_used_tokens,
                context_max_tokens = excluded.context_max_tokens,
                cache_creation_tokens = excluded.cache_creation_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                system_prompt_tokens = excluded.system_prompt_tokens",
            params![
                e.session_id,
                e.seq as i64,
                e.ts_ms as i64,
                bool_to_i64(e.ts_exact),
                format!("{:?}", e.kind),
                format!("{:?}", e.source),
                e.tool,
                e.tool_call_id,
                e.tokens_in.map(|v| v as i64),
                e.tokens_out.map(|v| v as i64),
                e.reasoning_tokens.map(|v| v as i64),
                e.cost_usd_e6,
                payload,
                e.stop_reason,
                e.latency_ms.map(|v| v as i64),
                e.ttft_ms.map(|v| v as i64),
                e.retry_count.map(|v| v as i64),
                e.context_used_tokens.map(|v| v as i64),
                e.context_max_tokens.map(|v| v as i64),
                e.cache_creation_tokens.map(|v| v as i64),
                e.cache_read_tokens.map(|v| v as i64),
                e.system_prompt_tokens.map(|v| v as i64),
            ],
        )?;
        if self.conn.changes() == 0 {
            return Ok(());
        }
        self.append_hot_event(e)?;
        if projector_legacy_mode() {
            index_event_derived(&self.conn, e)?;
            rebuild_tool_spans_for_session(&self.conn, &e.session_id)?;
            self.invalidate_span_tree_cache();
        } else if last_before.is_some_and(|last| e.seq <= last) {
            self.replay_projector_session(&e.session_id)?;
        } else {
            let deltas = self.projector.borrow_mut().apply(e);
            self.apply_projector_events(&deltas)?;
            let expired = self
                .projector
                .borrow_mut()
                .flush_expired(e.ts_ms, DEFAULT_ORPHAN_TTL_MS);
            self.apply_projector_events(&expired)?;
            if is_stop_event(e) {
                let flushed = self
                    .projector
                    .borrow_mut()
                    .flush_session(&e.session_id, e.ts_ms);
                self.apply_projector_events(&flushed)?;
            }
            self.invalidate_span_tree_cache();
        }
        self.append_search_event(e);
        self.refresh_extension_rows(e)?;
        let Some(ctx) = ctx else {
            return Ok(());
        };
        let sync = &ctx.sync;
        if sync.endpoint.is_empty() || sync.team_token.is_empty() || sync.team_id.is_empty() {
            return Ok(());
        }
        let Some(salt) = try_team_salt(sync) else {
            tracing::warn!(
                "sync outbox skipped: set sync.team_salt_hex (64 hex chars) in ~/.kaizen/config.toml"
            );
            return Ok(());
        };
        if sync.sample_rate < 1.0 {
            let u: f64 = rand::random();
            if u > sync.sample_rate {
                return Ok(());
            }
        }
        let Some(session) = self.get_session(&e.session_id)? else {
            tracing::warn!(session_id = %e.session_id, "sync outbox skipped: session not in DB");
            return Ok(());
        };
        let mut outbound = outbound_event_from_row(e, &session, &salt);
        redact_payload(&mut outbound.payload, ctx.workspace_root(), &salt);
        let row = serde_json::to_string(&outbound)?;
        self.outbox()?.append(&e.session_id, "events", &row)?;
        enqueue_tool_spans_for_session(self, &e.session_id, ctx)?;
        Ok(())
    }

    fn refresh_extension_rows(&self, e: &Event) -> Result<()> {
        crate::extensions::hash_chain::store_event_hash(self, e)?;
        crate::extensions::aggregates::upsert_session(self, &e.session_id)?;
        if let Err(err) = crate::extensions::diffs::refresh_session(self, &e.session_id, false) {
            tracing::warn!(session_id = %e.session_id, "step diff attribution skipped: {err:#}");
        }
        Ok(())
    }

    pub(super) fn append_hot_event(&self, e: &Event) -> Result<()> {
        if std::env::var("KAIZEN_HOT_LOG").as_deref() == Ok("0") {
            return Ok(());
        }
        let mut slot = self.hot_log.borrow_mut();
        if slot.is_none() {
            *slot = Some(HotLog::open(&self.root)?);
        }
        if let Some(log) = slot.as_mut() {
            log.append(e)?;
        }
        Ok(())
    }

    pub(super) fn append_search_event(&self, e: &Event) {
        if let Err(err) = self.try_append_search_event(e) {
            tracing::warn!(session_id = %e.session_id, seq = e.seq, "search index skipped: {err:#}");
            let _ = self.sync_state_set_u64(SYNC_STATE_SEARCH_DIRTY_MS, now_ms());
        }
    }

    pub(super) fn try_append_search_event(&self, e: &Event) -> Result<()> {
        let Some(session) = self.get_session(&e.session_id)? else {
            return Ok(());
        };
        let workspace = PathBuf::from(&session.workspace);
        let cfg = crate::core::config::load(&workspace).unwrap_or_default();
        let salt = try_team_salt(&cfg.sync).unwrap_or([0; 32]);
        let Some(doc) = crate::search::extract_doc(e, &session, &workspace, &salt) else {
            return Ok(());
        };
        let mut slot = self.search_writer.borrow_mut();
        if slot.is_none() {
            *slot = Some(crate::search::PendingWriter::open(&self.root)?);
        }
        slot.as_mut().expect("writer").add(&doc)
    }

    pub fn flush_search(&self) -> Result<()> {
        if let Some(writer) = self.search_writer.borrow_mut().as_mut() {
            writer.commit()?;
        }
        Ok(())
    }
}
