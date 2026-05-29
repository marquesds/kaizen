// SPDX-License-Identifier: AGPL-3.0-or-later

use super::*;

impl Store {
    pub fn files_for_session(&self, session_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM files_touched WHERE session_id = ?1 ORDER BY path ASC")?;
        let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0))?;
        Ok(rows.filter_map(|row| row.ok()).collect())
    }
}
