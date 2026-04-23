//! Spike E — SQLite WAL benchmark: 1k synthetic sessions, proxy H-queries.
//!
//! Generates sessions with random agent/model/duration/file lists,
//! runs proxy H2/H3/H8 queries, times each, prints table.
//! Done-signal: timing table printed; spike-e-ladybug.md committed with verdict.

use rand::{Rng, SeedableRng, rngs::SmallRng};
use rusqlite::{Connection, params};
use std::time::Instant;

const N_SESSIONS: usize = 1_000;
const SEED: u64 = 0xCAFE_BABE;

static AGENTS: &[&str] = &["cursor", "claude", "codex"];
static MODELS: &[&str] = &["gpt-4o", "claude-3-7-sonnet", "gpt-4.1", "o3"];
static FILES: &[&str] = &[
    "src/main.rs",
    "src/lib.rs",
    "src/core/session.rs",
    "src/collect/hooks/cursor.rs",
    "src/collect/hooks/claude.rs",
    "Cargo.toml",
    "README.md",
    "docs/impl-sequence.md",
];

fn setup_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
    conn.execute_batch(
        "CREATE TABLE sessions (
            id       TEXT PRIMARY KEY,
            agent    TEXT NOT NULL,
            model    TEXT NOT NULL,
            duration_s INTEGER NOT NULL
        );
        CREATE TABLE session_files (
            session_id TEXT NOT NULL,
            file       TEXT NOT NULL
        );",
    )
}

fn insert_sessions(conn: &Connection, rng: &mut SmallRng) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for i in 0..N_SESSIONS {
        let agent = AGENTS[rng.gen_range(0..AGENTS.len())];
        let model = MODELS[rng.gen_range(0..MODELS.len())];
        let dur: u32 = rng.gen_range(30..3600);
        let sid = format!("sess_{i:04}");
        tx.execute(
            "INSERT INTO sessions VALUES (?1,?2,?3,?4)",
            params![sid, agent, model, dur],
        )?;
        let n_files = rng.gen_range(1..6usize);
        for _ in 0..n_files {
            let f = FILES[rng.gen_range(0..FILES.len())];
            tx.execute("INSERT INTO session_files VALUES (?1,?2)", params![sid, f])?;
        }
    }
    tx.commit()
}

fn time_query(conn: &Connection, label: &str, sql: &str) {
    let t = Instant::now();
    let n: i64 = conn.query_row(sql, [], |row| row.get(0)).unwrap_or(0);
    let ms = t.elapsed().as_micros();
    println!("  {label:<30}  count={n:>6}  time={ms}µs");
}

fn main() {
    let mut rng = SmallRng::seed_from_u64(SEED);

    // Use in-memory DB for deterministic benchmark.
    let conn = Connection::open_in_memory().expect("open in-memory db");
    setup_schema(&conn).expect("schema");
    insert_sessions(&conn, &mut rng).expect("insert");

    println!("SQLite WAL benchmark — {N_SESSIONS} synthetic sessions\n");
    println!("  {:<30}  {:>6}  latency", "query", "rows");
    println!("  {}", "-".repeat(52));

    // H2: file co-edit pairs (sessions that touched ≥2 files).
    time_query(
        &conn,
        "H2 file_co_edit_count",
        "SELECT COUNT(DISTINCT a.session_id)
         FROM session_files a JOIN session_files b
           ON a.session_id = b.session_id AND a.file < b.file",
    );

    // H3: edit-loop detection (sessions with duration > 600s).
    time_query(
        &conn,
        "H3 edit_loops",
        "SELECT COUNT(*) FROM sessions WHERE duration_s > 600",
    );

    // H8: doc-edit co-occurrence (sessions with both .md and .rs files).
    time_query(
        &conn,
        "H8 doc_edit_cooccurrence",
        "SELECT COUNT(DISTINCT a.session_id)
         FROM session_files a JOIN session_files b
           ON a.session_id = b.session_id
          AND a.file LIKE '%.rs' AND b.file LIKE '%.md'",
    );

    println!("\nverdict: record outcome in docs/adr/001-storage.md (Spike E)");
}
