use super::SessionSummaryRead;
use super::session_search_sql;
use crate::store::Store;
use anyhow::{Result, ensure};
use rusqlite::named_params;

pub(crate) struct SessionSearchQuery {
    workspace: String,
    text: String,
    offset: i64,
    limit: i64,
    now_ms: i64,
    with_status: bool,
}

pub(super) struct SessionRowsPage {
    pub(super) rows: Vec<SessionSummaryRead>,
    pub(super) filtered_total: usize,
    pub(super) offset: usize,
    pub(super) limit: usize,
}

impl SessionSearchQuery {
    pub(crate) fn new(
        workspace: &str,
        text: &str,
        offset: usize,
        limit: usize,
        now_ms: u64,
    ) -> Result<Self> {
        validate(offset, limit, now_ms)?;
        Ok(valid_query(workspace, text, offset, limit, now_ms))
    }
}

pub(super) fn read(store: &Store, query: &SessionSearchQuery) -> Result<SessionRowsPage> {
    let filtered_total = count(store, query)?;
    Ok(SessionRowsPage {
        rows: rows(store, query)?,
        filtered_total,
        offset: query.offset as usize,
        limit: query.limit as usize,
    })
}

fn count(store: &Store, query: &SessionSearchQuery) -> Result<usize> {
    let count = if query.with_status {
        count_status(store, query)?
    } else {
        count_ordinary(store, query)?
    };
    Ok(count as usize)
}

fn rows(store: &Store, query: &SessionSearchQuery) -> Result<Vec<SessionSummaryRead>> {
    if query.with_status {
        rows_status(store, query)
    } else {
        rows_ordinary(store, query)
    }
}

fn count_status(store: &Store, query: &SessionSearchQuery) -> Result<i64> {
    let sql = session_search_sql::count(true);
    let values = named_params! {":workspace": query.workspace, ":text": query.text,
    ":now_ms": query.now_ms};
    Ok(store.conn().query_row(&sql, values, |row| row.get(0))?)
}

fn count_ordinary(store: &Store, query: &SessionSearchQuery) -> Result<i64> {
    let sql = session_search_sql::count(false);
    let values = named_params! {":workspace": query.workspace, ":text": query.text};
    Ok(store.conn().query_row(&sql, values, |row| row.get(0))?)
}

fn rows_status(store: &Store, query: &SessionSearchQuery) -> Result<Vec<SessionSummaryRead>> {
    let mut statement = store.conn().prepare(&session_search_sql::page(true))?;
    let values = named_params! {":workspace": query.workspace, ":text": query.text,
    ":now_ms": query.now_ms, ":limit": query.limit, ":offset": query.offset};
    let rows = statement.query_map(values, super::sessions::summary_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn rows_ordinary(store: &Store, query: &SessionSearchQuery) -> Result<Vec<SessionSummaryRead>> {
    let mut statement = store.conn().prepare(&session_search_sql::page(false))?;
    let values = named_params! {":workspace": query.workspace, ":text": query.text,
    ":limit": query.limit, ":offset": query.offset};
    let rows = statement.query_map(values, super::sessions::summary_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn searches_status(text: &str) -> bool {
    let text = caseless::default_case_fold_str(text);
    !text.is_empty()
        && ["active", "errored", "orphaned", "idle", "done"]
            .iter()
            .any(|status| status.contains(&text))
}

fn valid_query(
    workspace: &str,
    text: &str,
    offset: usize,
    limit: usize,
    now_ms: u64,
) -> SessionSearchQuery {
    SessionSearchQuery {
        workspace: workspace.into(),
        text: text.into(),
        offset: offset as i64,
        limit: limit as i64,
        now_ms: now_ms as i64,
        with_status: searches_status(text),
    }
}

fn validate(offset: usize, limit: usize, now_ms: u64) -> Result<()> {
    ensure!(limit > 0, "session page limit must be positive");
    ensure!(offset <= i64::MAX as usize, "session page offset too large");
    ensure!(limit <= i64::MAX as usize, "session page limit too large");
    ensure!(
        now_ms <= i64::MAX as u64,
        "session search timestamp too large"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::searches_status;

    #[test]
    fn status_path_only_for_visible_status_matches() {
        assert!(!searches_status(""));
        assert!(!searches_status("needle"));
        assert!(!searches_status("running"));
        assert!(searches_status("ctiv"));
        assert!(searches_status("IDLE"));
    }
}
