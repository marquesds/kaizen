use kaizen::DataSource;
use kaizen::experiment::store as exp_store;
use kaizen::experiment::types::{Binding, Criterion, Direction, Experiment, Metric, State};
use kaizen::search::reindex_workspace;
use kaizen::shell::cli::{sessions_list_text, summary_text};
use kaizen::shell::exp::{exp_power_text, exp_report_text};
use kaizen::shell::guidance::guidance_text;
use kaizen::shell::insights::insights_text;
use kaizen::shell::metrics::metrics_text;
use kaizen::shell::retro::retro_stdout;
use kaizen::shell::search::sessions_search_text;
use kaizen::store::Store;
use rusqlite::{Connection, params};
use std::time::{Duration, Instant};

const SESSIONS: usize = 10_000;
const EVENTS_PER_SESSION: usize = 100;
const WARM_BUDGET: Duration = Duration::from_millis(500);
type PerfCase<'a> = (&'a str, Box<dyn Fn() -> anyhow::Result<String> + 'a>);

#[test]
#[ignore = "perf harness seeds 10k sessions and 1M events"]
fn read_report_commands_warm_under_500ms() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let workspace = tmp.path();
    let db_path = workspace.join(".kaizen/kaizen.db");
    let ws = workspace.to_string_lossy().to_string();
    let store = Store::open(&db_path)?;
    seed(&db_path, &ws)?;
    seed_experiment(&store)?;
    let cfg = kaizen::core::config::Config::default();
    let sessions = store.list_sessions(&ws)?;
    let events = store.workspace_events(&ws)?;
    reindex_workspace(
        &workspace.join(".kaizen"),
        workspace,
        &sessions,
        events,
        &cfg,
    )?;

    let cases: Vec<PerfCase<'_>> = vec![
        (
            "retro",
            Box::new(|| {
                retro_stdout(
                    Some(workspace),
                    7,
                    true,
                    false,
                    false,
                    false,
                    DataSource::Local,
                )
            }),
        ),
        (
            "exp power",
            Box::new(|| exp_power_text(Some(workspace), "tokens_per_session", 50, false)),
        ),
        (
            "exp report",
            Box::new(|| exp_report_text(Some(workspace), "perf-exp", false, false)),
        ),
        (
            "summary",
            Box::new(|| summary_text(Some(workspace), false, false, false, DataSource::Local)),
        ),
        (
            "insights",
            Box::new(|| insights_text(Some(workspace), false, false, DataSource::Local)),
        ),
        (
            "metrics",
            Box::new(|| {
                metrics_text(
                    Some(workspace),
                    7,
                    false,
                    false,
                    false,
                    false,
                    DataSource::Local,
                )
            }),
        ),
        (
            "guidance",
            Box::new(|| guidance_text(Some(workspace), 7, false, false, DataSource::Local)),
        ),
        (
            "sessions list",
            Box::new(|| sessions_list_text(Some(workspace), false, false, false, None)),
        ),
        (
            "sessions search",
            Box::new(|| sessions_search_text(Some(workspace), "read_file", None, None, None, 10)),
        ),
    ];
    for (name, run) in cases {
        let elapsed = time(&run)?;
        eprintln!("{name}: {:.1}ms", elapsed.as_secs_f64() * 1000.0);
        assert!(
            elapsed < WARM_BUDGET,
            "{name} exceeded warm budget: {elapsed:?}"
        );
    }
    Ok(())
}

fn time(run: &dyn Fn() -> anyhow::Result<String>) -> anyhow::Result<Duration> {
    let start = Instant::now();
    let _ = run()?;
    Ok(start.elapsed())
}

fn seed_experiment(store: &Store) -> anyhow::Result<()> {
    exp_store::save_experiment(
        store,
        &Experiment {
            id: "perf-exp".into(),
            name: "perf".into(),
            hypothesis: "faster reads".into(),
            change_description: "cache-first".into(),
            metric: Metric::TokensPerSession,
            binding: Binding::ManualTag {
                variant_field: "variant".into(),
            },
            duration_days: 14,
            success_criterion: Criterion::Delta {
                direction: Direction::Decrease,
                target_pct: -10.0,
            },
            state: State::Running,
            created_at_ms: 1_700_000_000_000,
            concluded_at_ms: Some(1_700_086_400_000),
            guardrails: Vec::new(),
        },
    )
}

fn seed(db_path: &std::path::Path, workspace: &str) -> anyhow::Result<()> {
    let mut conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA synchronous=OFF; PRAGMA temp_store=MEMORY;")?;
    let tx = conn.transaction()?;
    let mut session_stmt = tx.prepare(
        "INSERT INTO sessions
         (id, agent, model, workspace, started_at_ms, ended_at_ms, status, trace_path)
         VALUES (?1, 'codex', 'gpt', ?2, ?3, ?4, 'Done', ?5)",
    )?;
    let mut event_stmt = tx.prepare(
        "INSERT INTO events
         (session_id, seq, ts_ms, kind, source, tool, tokens_in, tokens_out, reasoning_tokens, cost_usd_e6, payload)
         VALUES (?1, ?2, ?3, 'ToolCall', 'Tail', ?4, 100, 50, 20, ?5, '{}')",
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
        for e in 0..EVENTS_PER_SESSION {
            event_stmt.execute(params![
                sid,
                e as i64,
                started + e as i64,
                "read_file",
                e as i64
            ])?;
        }
    }
    drop(event_stmt);
    drop(session_stmt);
    tx.commit()?;
    Ok(())
}
