// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tantivy schema for event search.

use tantivy::schema::{
    FAST, Field, INDEXED, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions,
};

#[derive(Clone, Copy)]
pub struct SearchFields {
    pub session_id: Field,
    pub seq: Field,
    pub event_key: Field,
    pub ts_ms: Field,
    pub agent: Field,
    pub kind: Field,
    pub text: Field,
    pub path: Field,
    pub skill: Field,
    pub tokens_total: Field,
}

pub fn build_schema() -> (Schema, SearchFields) {
    let mut b = Schema::builder();
    let session_id = b.add_text_field("session_id", STRING | STORED);
    let seq = b.add_i64_field("seq", INDEXED | FAST | STORED);
    let event_key = b.add_text_field("event_key", STRING);
    let ts_ms = b.add_i64_field("ts_ms", INDEXED | FAST | STORED);
    let agent = b.add_text_field("agent", STRING | STORED);
    let kind = b.add_text_field("kind", STRING | STORED);
    let text = b.add_text_field("text", bm25_text());
    let path = b.add_text_field("path", STRING);
    let skill = b.add_text_field("skill", STRING);
    let tokens_total = b.add_i64_field("tokens_total", INDEXED | FAST | STORED);
    let schema = b.build();
    let fields = SearchFields {
        session_id,
        seq,
        event_key,
        ts_ms,
        agent,
        kind,
        text,
        path,
        skill,
        tokens_total,
    };
    (schema, fields)
}

fn bm25_text() -> TextOptions {
    TextOptions::default().set_indexing_options(
        TextFieldIndexing::default().set_index_option(IndexRecordOption::WithFreqsAndPositions),
    )
}

pub fn event_key(session_id: &str, seq: u64) -> String {
    format!("{session_id}\0{seq}")
}
