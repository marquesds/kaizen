// SPDX-License-Identifier: AGPL-3.0-or-later
//! Analytics query facade. DuckDB scans cold Parquet; SQLite remains warm detail store.

use crate::store::sqlite::{Store, SummaryStats};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct QueryStore {
    root: PathBuf,
}

impl QueryStore {
    pub fn open(root: &Path) -> Result<Self> {
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn summary_stats(&self, sqlite: &Store, workspace: &str) -> Result<SummaryStats> {
        let mut stats = sqlite.summary_stats(workspace)?;
        #[cfg(feature = "analytics-duckdb")]
        {
            if self.events_glob_exists() {
                let duck = duckdb::Connection::open_in_memory()?;
                let glob = sql_string(&self.events_glob());
                let cost: i64 = duck.query_row(
                    &format!("SELECT COALESCE(SUM(cost_usd_e6), 0) FROM read_parquet({glob})"),
                    [],
                    |r| r.get(0),
                )?;
                stats.total_cost_usd_e6 = stats.total_cost_usd_e6.saturating_add(cost);
                stats.top_tools = merge_top_tools(
                    stats.top_tools,
                    cold_top_tools(&duck, &glob).unwrap_or_default(),
                );
            }
        }
        Ok(stats)
    }

    pub fn cold_event_count(&self) -> Result<u64> {
        #[cfg(feature = "analytics-duckdb")]
        {
            if !self.events_glob_exists() {
                return Ok(0);
            }
            let duck = duckdb::Connection::open_in_memory()?;
            let sql = format!(
                "SELECT COUNT(*) FROM read_parquet({})",
                sql_string(&self.events_glob())
            );
            let n: i64 = duck.query_row(&sql, [], |r| r.get(0))?;
            Ok(n as u64)
        }
        #[cfg(not(feature = "analytics-duckdb"))]
        {
            Ok(0)
        }
    }

    fn events_glob(&self) -> String {
        self.root
            .join("cold/events/*.parquet")
            .to_string_lossy()
            .to_string()
    }

    fn events_glob_exists(&self) -> bool {
        self.root.join("cold/events").exists()
    }
}

#[cfg(feature = "analytics-duckdb")]
fn cold_top_tools(duck: &duckdb::Connection, glob: &str) -> Result<Vec<(String, u64)>> {
    let sql = format!(
        "SELECT tool, COUNT(*) FROM read_parquet({glob}) \
         WHERE tool IS NOT NULL GROUP BY tool ORDER BY COUNT(*) DESC LIMIT 10"
    );
    let mut stmt = duck.prepare(&sql)?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u64))
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn merge_top_tools(mut warm: Vec<(String, u64)>, cold: Vec<(String, u64)>) -> Vec<(String, u64)> {
    for (tool, n) in cold {
        if let Some((_, total)) = warm.iter_mut().find(|(t, _)| t == &tool) {
            *total += n;
        } else {
            warm.push((tool, n));
        }
    }
    warm.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    warm.truncate(10);
    warm
}

fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
