use anyhow::Result;
use rusqlite::Connection;
use rusqlite::functions::FunctionFlags;

pub(super) fn register(conn: &Connection) -> Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    conn.create_scalar_function("kaizen_casefold", 1, flags, |context| {
        let value = context.get::<String>(0)?;
        Ok(caseless::default_case_fold_str(&value))
    })?;
    Ok(())
}
