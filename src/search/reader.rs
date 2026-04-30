// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tantivy reader and result shaping.

use crate::core::event::Event;
use crate::search::extract::{redacted_event_text, snippet, tokens_total};
use crate::search::schema::{SearchFields, build_schema};
use crate::search::writer::index_dir;
use anyhow::Result;
use serde_json::Value as JsonValue;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{TantivyDocument, Value};
use tantivy::{Index, TantivyError};

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub since_ms: Option<u64>,
    pub agent: Option<String>,
    pub kind: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub session_id: String,
    pub seq: u64,
    pub ts_ms: u64,
    pub agent: String,
    pub kind: String,
    pub score: f32,
    pub snippet: String,
    pub paths: Vec<String>,
    pub skills: Vec<String>,
    pub tokens_total: i64,
}

pub fn search<F>(
    root: &Path,
    opts: &SearchQuery,
    workspace: &Path,
    salt: &[u8; 32],
    load: F,
) -> Result<Vec<SearchHit>>
where
    F: Fn(&str, u64) -> Result<Option<Event>>,
{
    let (_, fields) = build_schema();
    let index = Index::open_in_dir(index_dir(root))?;
    let parser = QueryParser::for_index(&index, vec![fields.text]);
    let query = parser.parse_query(&query_text(opts))?;
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let docs = searcher.search(&query, &TopDocs::with_limit(opts.limit).order_by_score())?;
    let ctx = HitCtx {
        fields,
        opts,
        workspace,
        salt,
    };
    docs.into_iter()
        .filter_map(|(score, addr)| doc_hit(&searcher, addr, score, &ctx, &load))
        .collect()
}

pub fn is_missing_index(err: &anyhow::Error) -> bool {
    err.downcast_ref::<TantivyError>().is_some()
}

fn query_text(opts: &SearchQuery) -> String {
    [
        opts.query.clone(),
        opt("agent", &opts.agent),
        opt("kind", &opts.kind),
    ]
    .into_iter()
    .chain(opts.since_ms.map(|ms| format!("ts_ms:>={ms}")))
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" AND ")
}

fn opt(field: &str, value: &Option<String>) -> String {
    value
        .as_ref()
        .map(|v| format!("{field}:{v}"))
        .unwrap_or_default()
}

struct HitCtx<'a> {
    fields: SearchFields,
    opts: &'a SearchQuery,
    workspace: &'a Path,
    salt: &'a [u8; 32],
}

fn doc_hit<F>(
    searcher: &tantivy::Searcher,
    addr: tantivy::DocAddress,
    score: f32,
    ctx: &HitCtx<'_>,
    load: &F,
) -> Option<Result<SearchHit>>
where
    F: Fn(&str, u64) -> Result<Option<Event>>,
{
    Some((|| {
        let doc = searcher.doc::<TantivyDocument>(addr)?;
        let session_id = str_field(&doc, ctx.fields.session_id)?;
        let seq = i64_field(&doc, ctx.fields.seq)? as u64;
        let event = load(&session_id, seq)?.unwrap_or_else(|| empty_event(&session_id, seq));
        Ok(SearchHit {
            session_id,
            seq,
            ts_ms: i64_field(&doc, ctx.fields.ts_ms)? as u64,
            agent: str_field(&doc, ctx.fields.agent)?,
            kind: str_field(&doc, ctx.fields.kind)?,
            score,
            snippet: snippet(
                &redacted_event_text(&event, ctx.workspace, ctx.salt),
                &ctx.opts.query,
            ),
            paths: crate::store::event_index::paths_from_event_payload(&event.payload),
            skills: crate::store::event_index::skills_from_event_json(&event.payload),
            tokens_total: tokens_total(&event),
        })
    })())
}

fn str_field(doc: &TantivyDocument, field: tantivy::schema::Field) -> Result<String> {
    Ok(doc
        .get_first(field)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string())
}

fn i64_field(doc: &TantivyDocument, field: tantivy::schema::Field) -> Result<i64> {
    Ok(doc
        .get_first(field)
        .and_then(|v| v.as_i64())
        .unwrap_or_default())
}

fn empty_event(session_id: &str, seq: u64) -> Event {
    Event {
        session_id: session_id.to_string(),
        seq,
        ts_ms: 0,
        ts_exact: false,
        kind: crate::core::event::EventKind::Message,
        source: crate::core::event::EventSource::Tail,
        tool: None,
        tool_call_id: None,
        tokens_in: None,
        tokens_out: None,
        reasoning_tokens: None,
        cost_usd_e6: None,
        stop_reason: None,
        latency_ms: None,
        ttft_ms: None,
        retry_count: None,
        context_used_tokens: None,
        context_max_tokens: None,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        system_prompt_tokens: None,
        payload: JsonValue::Null,
    }
}
