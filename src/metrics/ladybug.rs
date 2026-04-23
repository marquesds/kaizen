// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ladybug sidecar. Derived only. Safe to rebuild.

use crate::metrics::types::{RepoEdge, RepoSnapshotRecord, SymbolFact};
use anyhow::Result;
use lbug::{Connection, Database, SystemConfig};
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
    let db = Database::new(graph_path, SystemConfig::default())?;
    let conn = Connection::new(&db)?;
    create_schema(&conn)?;
    conn.query(&format!(
        "CREATE (:Snapshot {{id: '{}', workspace: '{}', head_commit: '{}', analyzer_version: '{}', indexed_at_ms: {}}});",
        esc(&snapshot.id),
        esc(&snapshot.workspace),
        esc(snapshot.head_commit.as_deref().unwrap_or("")),
        esc(&snapshot.analyzer_version),
        snapshot.indexed_at_ms
    ))?;
    for file in files {
        conn.query(&format!(
            "CREATE (:File {{id: '{id}', path: '{path}', language: '{lang}', complexity: {cx}, churn30: {churn}}});",
            id = esc(&file.path),
            path = esc(&file.path),
            lang = esc(&file.language),
            cx = file.complexity_total,
            churn = file.churn_30d
        ))?;
        conn.query(&format!(
            "MATCH (s:Snapshot {{id: '{sid}'}}), (f:File {{id: '{fid}'}}) CREATE (s)-[:CONTAINS]->(f);",
            sid = esc(&snapshot.id),
            fid = esc(&file.path)
        ))?;
    }
    for symbol in symbols {
        let sym_id = symbol_id(symbol);
        conn.query(&format!(
            "CREATE (:Symbol {{id: '{id}', path: '{path}', name: '{name}', kind: '{kind}', complexity: {cx}}});",
            id = esc(&sym_id),
            path = esc(&symbol.path),
            name = esc(&symbol.name),
            kind = esc(&symbol.kind),
            cx = symbol.complexity
        ))?;
        conn.query(&format!(
            "MATCH (f:File {{id: '{fid}'}}), (s:Symbol {{id: '{sid}'}}) CREATE (f)-[:DECLARES]->(s);",
            fid = esc(&symbol.path),
            sid = esc(&sym_id)
        ))?;
    }
    for edge in edges {
        if edge.kind == "CALLS" {
            conn.query(&format!(
                "MATCH (a:Symbol {{id: '{a}'}}), (b:Symbol {{id: '{b}'}}) CREATE (a)-[:CALLS {{weight: {w}}}]->(b);",
                a = esc(&edge.from_path),
                b = esc(&edge.to_path),
                w = edge.weight
            ))?;
            continue;
        }
        conn.query(&format!(
            "MATCH (a:File {{id: '{a}'}}), (b:File {{id: '{b}'}}) CREATE (a)-[:{kind} {{weight: {w}}}]->(b);",
            a = esc(&edge.from_path),
            b = esc(&edge.to_path),
            kind = edge.kind,
            w = edge.weight
        ))?;
    }
    Ok(())
}

fn create_schema(conn: &Connection<'_>) -> Result<()> {
    conn.query("CREATE NODE TABLE Snapshot(id STRING PRIMARY KEY, workspace STRING, head_commit STRING, analyzer_version STRING, indexed_at_ms INT64);")?;
    conn.query("CREATE NODE TABLE File(id STRING PRIMARY KEY, path STRING, language STRING, complexity INT64, churn30 INT64);")?;
    conn.query("CREATE NODE TABLE Symbol(id STRING PRIMARY KEY, path STRING, name STRING, kind STRING, complexity INT64);")?;
    conn.query("CREATE REL TABLE CONTAINS(FROM Snapshot TO File);")?;
    conn.query("CREATE REL TABLE DECLARES(FROM File TO Symbol);")?;
    conn.query("CREATE REL TABLE IMPORTS(FROM File TO File, weight INT64);")?;
    conn.query("CREATE REL TABLE DEPENDS_ON(FROM File TO File, weight INT64);")?;
    conn.query("CREATE REL TABLE CO_CHANGED_WITH(FROM File TO File, weight INT64);")?;
    conn.query("CREATE REL TABLE CALLS(FROM Symbol TO Symbol, weight INT64);")?;
    Ok(())
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
