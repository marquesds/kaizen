// SPDX-License-Identifier: AGPL-3.0-or-later

mod read;
mod write;

use super::final_span_records;
use anyhow::Result;
use rusqlite::Connection;

pub(crate) use write::{clear_session_spans, upsert_tool_span_record};

pub fn rebuild_tool_spans_for_session(conn: &Connection, session_id: &str) -> Result<()> {
    let events = read::load_session_events(conn, session_id)?;
    clear_session_spans(conn, session_id)?;
    for span in final_span_records(&events) {
        upsert_tool_span_record(conn, &span)?;
    }
    Ok(())
}
