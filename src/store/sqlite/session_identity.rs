use anyhow::Result;
use rusqlite::Connection;

const MARKER: &str = "session_identity_backfill_v1";
const REPAIR: &str = r#"
UPDATE sessions AS s SET
 agent = CASE WHEN lower(s.agent) = 'claude' AND (
  EXISTS(SELECT 1 FROM events e WHERE e.session_id = s.id AND
   (json_type(e.payload, '$.turn_id') IS NOT NULL OR
    lower(COALESCE(json_extract(e.payload, '$.transcript_path'), '')) LIKE '%/.codex/%'))
  OR lower(COALESCE(s.model, '')) GLOB 'gpt-*'
  OR lower(COALESCE(s.model, '')) GLOB '*codex*'
  OR lower(COALESCE(s.model, '')) GLOB 'kindle-*'
  OR lower(COALESCE(s.model, '')) GLOB 'nova-*'
 ) THEN 'codex' ELSE s.agent END,
 model = COALESCE(s.model, (SELECT json_extract(e.payload, '$.model') FROM events e
  WHERE e.session_id = s.id AND json_type(e.payload, '$.model') = 'text'
  ORDER BY e.seq DESC LIMIT 1)),
 trace_path = CASE WHEN s.trace_path = '' THEN COALESCE((SELECT
  json_extract(e.payload, '$.transcript_path') FROM events e WHERE e.session_id = s.id
  AND json_type(e.payload, '$.transcript_path') = 'text' ORDER BY e.seq DESC LIMIT 1), '')
  ELSE s.trace_path END
WHERE EXISTS(SELECT 1 FROM events e WHERE e.session_id = s.id AND
 (json_type(e.payload, '$.turn_id') IS NOT NULL OR json_type(e.payload, '$.model') = 'text'
  OR json_type(e.payload, '$.transcript_path') = 'text'))
"#;

pub(super) fn backfill(conn: &Connection) -> Result<()> {
    if empty(conn)? || complete(conn)? {
        return Ok(());
    }
    conn.execute(REPAIR, [])?;
    conn.execute(
        "INSERT OR REPLACE INTO sync_state(k, v) VALUES (?1, 'done')",
        [MARKER],
    )?;
    Ok(())
}

fn empty(conn: &Connection) -> Result<bool> {
    Ok(
        conn.query_row("SELECT NOT EXISTS(SELECT 1 FROM sessions)", [], |row| {
            row.get(0)
        })?,
    )
}

fn complete(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_state WHERE k = ?1)",
        [MARKER],
        |row| row.get(0),
    )?)
}
