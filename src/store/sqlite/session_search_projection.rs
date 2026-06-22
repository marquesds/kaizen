use super::*;

const UPSERT: &str = "INSERT INTO session_search_prompts (session_id, event_seq, prompt)
 VALUES (?1, ?2, ?3) ON CONFLICT(session_id) DO UPDATE SET
 event_seq = excluded.event_seq, prompt = excluded.prompt
 WHERE excluded.event_seq < session_search_prompts.event_seq";

const BACKFILL: &str = "SELECT e.session_id, e.seq, e.payload FROM events e
 LEFT JOIN session_search_prompts p ON p.session_id = e.session_id
 WHERE p.session_id IS NULL ORDER BY e.session_id, e.seq";

pub(super) fn backfill(conn: &Connection) -> Result<()> {
    backfill_prompts(conn)?;
    fill_empty(conn)
}

fn backfill_prompts(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare(BACKFILL)?;
    let mut rows = statement.query([])?;
    project_rows(conn, &mut rows)
}

fn project_rows(conn: &Connection, rows: &mut rusqlite::Rows<'_>) -> Result<()> {
    while let Some(row) = rows.next()? {
        let payload: String = row.get(2)?;
        project_raw(conn, row.get(0)?, row.get(1)?, &payload)?;
    }
    Ok(())
}

pub(super) fn project(conn: &Connection, event: &Event) -> Result<()> {
    let Some(prompt) = crate::core::prompt_text::from_value(&event.payload) else {
        return Ok(());
    };
    upsert(conn, &event.session_id, event.seq as i64, &prompt)
}

fn project_raw(conn: &Connection, id: String, seq: i64, raw: &str) -> Result<()> {
    let Some(prompt) = serde_json::from_str(raw)
        .ok()
        .as_ref()
        .and_then(crate::core::prompt_text::from_value)
    else {
        return Ok(());
    };
    upsert(conn, &id, seq, &prompt)
}

fn upsert(conn: &Connection, id: &str, seq: i64, prompt: &str) -> Result<()> {
    conn.execute(UPSERT, params![id, seq, prompt])?;
    Ok(())
}

fn fill_empty(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT INTO session_search_prompts (session_id, event_seq, prompt)
         SELECT s.id, ?1, '' FROM sessions s LEFT JOIN session_search_prompts p
         ON p.session_id = s.id WHERE p.session_id IS NULL",
        [i64::MAX],
    )?;
    Ok(())
}
