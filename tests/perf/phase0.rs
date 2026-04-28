use kaizen::core::data_source::DataSource;
use kaizen::shell::cli::summary_text;
use kaizen::store::Store;
use rusqlite::{Connection, params};
use std::time::{Duration, Instant};

const SESSIONS: usize = 10_000;
const EVENTS_PER_SESSION: usize = 100;

#[test]
#[ignore = "perf harness seeds 10k sessions and 1M events"]
fn phase0_perf_harness() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path();
    let db_path = workspace.join(".kaizen/kaizen.db");
    Store::open(&db_path)?;
    seed(&db_path, &workspace.to_string_lossy())?;

    let start = Instant::now();
    let store = Store::open_read_only(&db_path)?;
    let sessions = store.list_sessions(&workspace.to_string_lossy())?;
    let cold_start = start.elapsed();

    let ids = sessions.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let mut refresh_samples = Vec::new();
    for _ in 0..100 {
        let t = Instant::now();
        let _ = store.list_sessions_started_after(&workspace.to_string_lossy(), u64::MAX - 1)?;
        let _ = store.session_statuses(&ids)?;
        let _ = store.list_events_for_session(&sessions[0].id)?;
        refresh_samples.push(t.elapsed());
    }
    refresh_samples.sort();
    let refresh_p99 = refresh_samples[98];

    let summary_start = Instant::now();
    let _ = summary_text(Some(workspace), false, false, false, DataSource::Local)?;
    let summary_runtime = summary_start.elapsed();

    eprintln!("phase0 perf:");
    eprintln!("  sessions: {SESSIONS}");
    eprintln!("  events: {}", SESSIONS * EVENTS_PER_SESSION);
    eprintln!("  TUI cold start: {}", ms(cold_start));
    eprintln!("  refresh p99: {}", ms(refresh_p99));
    eprintln!("  summary runtime: {}", ms(summary_runtime));
    Ok(())
}

fn seed(db_path: &std::path::Path, workspace: &str) -> anyhow::Result<()> {
    let mut conn = Connection::open(db_path)?;
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=OFF;
        PRAGMA temp_store=MEMORY;
        ",
    )?;
    let tx = conn.transaction()?;
    {
        let mut session_stmt = tx.prepare(
            "INSERT INTO sessions
             (id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path)
             VALUES (?1, 'codex', 'gpt', ?2, ?3, ?4, 'Done', ?5)",
        )?;
        let mut event_stmt = tx.prepare(
            "INSERT INTO events
             (session_id, seq, ts_ms, kind, source, tool, cost_usd_e6, payload)
             VALUES (?1, ?2, ?3, 'ToolCall', 'Tail', ?4, ?5, '{}')",
        )?;
        let mut span_stmt = tx.prepare(
            "INSERT INTO tool_spans
             (span_id, session_id, tool, status, started_at_ms, ended_at_ms,
              lead_time_ms, tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, paths_json)
             VALUES (?1, ?2, ?3, 'done', ?4, ?5, 10, 100, 50, 20, ?6, '[]')",
        )?;
        let mut sample_stmt = tx.prepare(
            "INSERT INTO session_samples
             (session_id, ts_ms, pid, cpu_percent, rss_bytes)
             VALUES (?1, ?2, 1, 0.0, 1024)",
        )?;
        for s in 0..SESSIONS {
            let sid = format!("s{s:05}");
            let started = 1_700_000_000_000_i64 + s as i64;
            session_stmt.execute(params![
                sid,
                workspace,
                started,
                started + 60_000,
                format!("/trace/{s}")
            ])?;
            sample_stmt.execute(params![sid, started])?;
            for e in 0..EVENTS_PER_SESSION {
                let ts = started + e as i64;
                let tool = if e % 2 == 0 { "read_file" } else { "bash" };
                event_stmt.execute(params![sid, e as i64, ts, tool, e as i64])?;
                span_stmt.execute(params![
                    format!("{sid}-{e}"),
                    sid,
                    tool,
                    ts,
                    ts + 10,
                    e as i64
                ])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

fn ms(duration: Duration) -> String {
    format!("{:.1}ms", duration.as_secs_f64() * 1_000.0)
}
