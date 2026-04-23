// SPDX-License-Identifier: AGPL-3.0-or-later
//! Code graph sidecar: SQLite file with the GraphQLite extension (Cypher).
//! Derived only. Safe to rebuild.

use crate::metrics::types::{RepoEdge, RepoSnapshotRecord, SymbolFact};
use anyhow::{Context, Result};
use graphqlite::Connection;
use std::fs;
use std::path::Path;

pub fn rebuild_sidecar(
    graph_path: &Path,
    snapshot: &RepoSnapshotRecord,
    files: &[crate::metrics::types::FileFact],
    symbols: &[SymbolFact],
    edges: &[RepoEdge],
) -> Result<()> {
    if graph_path.exists() {
        let _ = fs::remove_file(graph_path);
    }
    let conn = Connection::open(graph_path).context("open codegraph database")?;

    run_cypher(
        &conn,
        &format!(
            "CREATE (s:Snapshot {{id: '{}', workspace: '{}', head_commit: '{}', analyzer_version: '{}', indexed_at_ms: {}}})",
            esc(&snapshot.id),
            esc(&snapshot.workspace),
            esc(snapshot.head_commit.as_deref().unwrap_or("")),
            esc(&snapshot.analyzer_version),
            snapshot.indexed_at_ms
        ),
    )?;

    for file in files {
        run_cypher(
            &conn,
            &format!(
                "CREATE (f:File {{id: '{}', path: '{}', language: '{}', complexity: {}, churn30: {}}})",
                esc(&file.path),
                esc(&file.path),
                esc(&file.language),
                file.complexity_total,
                file.churn_30d
            ),
        )?;
        run_cypher(
            &conn,
            &format!(
                "MATCH (s:Snapshot {{id: '{}'}}), (f:File {{id: '{}'}}) CREATE (s)-[:CONTAINS]->(f)",
                esc(&snapshot.id),
                esc(&file.path)
            ),
        )?;
    }

    for symbol in symbols {
        let sym_id = symbol_id(symbol);
        run_cypher(
            &conn,
            &format!(
                "CREATE (sym:Symbol {{id: '{}', path: '{}', name: '{}', kind: '{}', complexity: {}}})",
                esc(&sym_id),
                esc(&symbol.path),
                esc(&symbol.name),
                esc(&symbol.kind),
                symbol.complexity
            ),
        )?;
        run_cypher(
            &conn,
            &format!(
                "MATCH (fb:File {{id: '{}'}}), (sm:Symbol {{id: '{}'}}) CREATE (fb)-[:DECLARES]->(sm)",
                esc(&symbol.path),
                esc(&sym_id)
            ),
        )?;
    }

    for edge in edges {
        if edge.kind == "CALLS" {
            run_cypher(
                &conn,
                &format!(
                    "MATCH (a:Symbol {{id: '{}'}}), (b:Symbol {{id: '{}'}}) CREATE (a)-[:CALLS {{weight: {}}}]->(b)",
                    esc(&edge.from_path),
                    esc(&edge.to_path),
                    edge.weight
                ),
            )?;
            continue;
        }
        run_cypher(
            &conn,
            &format!(
                "MATCH (a:File {{id: '{}'}}), (b:File {{id: '{}'}}) CREATE (a)-[:{} {{weight: {}}}]->(b)",
                esc(&edge.from_path),
                esc(&edge.to_path),
                edge.kind,
                edge.weight
            ),
        )?;
    }

    Ok(())
}

fn run_cypher(conn: &Connection, query: &str) -> Result<()> {
    conn.cypher(query)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("{e}"))
}

pub fn symbol_id(symbol: &SymbolFact) -> String {
    format!(
        "{}#{}:{}-{}",
        symbol.path, symbol.name, symbol.start_byte, symbol.end_byte
    )
}

fn esc(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\'', "\\'")
}
