use super::*;

pub(super) const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        agent TEXT NOT NULL,
        model TEXT,
        workspace TEXT NOT NULL,
        started_at_ms INTEGER NOT NULL,
        ended_at_ms INTEGER,
        status TEXT NOT NULL,
        trace_path TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        seq INTEGER NOT NULL,
        ts_ms INTEGER NOT NULL,
        kind TEXT NOT NULL,
        source TEXT NOT NULL,
        tool TEXT,
        tokens_in INTEGER,
        tokens_out INTEGER,
        cost_usd_e6 INTEGER,
        payload TEXT NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS events_session_idx ON events(session_id)",
    "CREATE TABLE IF NOT EXISTS files_touched (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        path TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS skills_used (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        skill TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS sync_outbox (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        payload TEXT NOT NULL,
        sent INTEGER NOT NULL DEFAULT 0
    )",
    "CREATE TABLE IF NOT EXISTS experiments (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL,
        metadata TEXT NOT NULL DEFAULT '{}'
    )",
    "CREATE TABLE IF NOT EXISTS experiment_tags (
        experiment_id TEXT NOT NULL,
        session_id TEXT NOT NULL,
        variant TEXT NOT NULL,
        PRIMARY KEY (experiment_id, session_id)
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS events_session_seq_idx ON events(session_id, seq)",
    "CREATE TABLE IF NOT EXISTS sync_state (
        k TEXT PRIMARY KEY,
        v TEXT NOT NULL
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS files_touched_session_path_idx ON files_touched(session_id, path)",
    "CREATE UNIQUE INDEX IF NOT EXISTS skills_used_session_skill_idx ON skills_used(session_id, skill)",
    "CREATE TABLE IF NOT EXISTS tool_spans (
        span_id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        tool TEXT,
        tool_call_id TEXT,
        status TEXT NOT NULL,
        started_at_ms INTEGER,
        ended_at_ms INTEGER,
        lead_time_ms INTEGER,
        tokens_in INTEGER,
        tokens_out INTEGER,
        reasoning_tokens INTEGER,
        cost_usd_e6 INTEGER,
        paths_json TEXT NOT NULL DEFAULT '[]'
    )",
    "CREATE TABLE IF NOT EXISTS tool_span_paths (
        span_id TEXT NOT NULL,
        path TEXT NOT NULL,
        PRIMARY KEY (span_id, path)
    )",
    "CREATE TABLE IF NOT EXISTS trace_spans (
        span_id TEXT PRIMARY KEY,
        trace_id TEXT NOT NULL,
        parent_span_id TEXT,
        session_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        name TEXT NOT NULL,
        status TEXT NOT NULL,
        started_at_ms INTEGER,
        ended_at_ms INTEGER,
        duration_ms INTEGER,
        model TEXT,
        tool TEXT,
        tokens_in INTEGER,
        tokens_out INTEGER,
        reasoning_tokens INTEGER,
        cost_usd_e6 INTEGER,
        context_used_tokens INTEGER,
        context_max_tokens INTEGER,
        payload TEXT NOT NULL DEFAULT '{}'
    )",
    "CREATE INDEX IF NOT EXISTS trace_spans_session_idx ON trace_spans(session_id)",
    "CREATE INDEX IF NOT EXISTS trace_spans_trace_idx ON trace_spans(trace_id)",
    "CREATE INDEX IF NOT EXISTS trace_spans_started_idx ON trace_spans(started_at_ms)",
    "CREATE TABLE IF NOT EXISTS session_repo_binding (
        session_id TEXT PRIMARY KEY,
        start_commit TEXT,
        end_commit TEXT,
        branch TEXT,
        dirty_start INTEGER,
        dirty_end INTEGER,
        repo_binding_source TEXT NOT NULL DEFAULT ''
    )",
    "CREATE TABLE IF NOT EXISTS repo_snapshots (
        id TEXT PRIMARY KEY,
        workspace TEXT NOT NULL,
        head_commit TEXT,
        dirty_fingerprint TEXT NOT NULL,
        analyzer_version TEXT NOT NULL,
        indexed_at_ms INTEGER NOT NULL,
        dirty INTEGER NOT NULL DEFAULT 0,
        graph_path TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS file_facts (
        snapshot_id TEXT NOT NULL,
        path TEXT NOT NULL,
        language TEXT NOT NULL,
        bytes INTEGER NOT NULL,
        loc INTEGER NOT NULL,
        sloc INTEGER NOT NULL,
        complexity_total INTEGER NOT NULL,
        max_fn_complexity INTEGER NOT NULL,
        symbol_count INTEGER NOT NULL,
        import_count INTEGER NOT NULL,
        fan_in INTEGER NOT NULL,
        fan_out INTEGER NOT NULL,
        churn_30d INTEGER NOT NULL,
        churn_90d INTEGER NOT NULL,
        authors_90d INTEGER NOT NULL,
        last_changed_ms INTEGER,
        PRIMARY KEY (snapshot_id, path)
    )",
    "CREATE TABLE IF NOT EXISTS repo_edges (
        snapshot_id TEXT NOT NULL,
        from_id TEXT NOT NULL,
        to_id TEXT NOT NULL,
        kind TEXT NOT NULL,
        weight INTEGER NOT NULL,
        PRIMARY KEY (snapshot_id, from_id, to_id, kind)
    )",
    // Speed workspace-scoped `insights` / `summary` (sessions filter before joining events)
    "CREATE INDEX IF NOT EXISTS sessions_workspace_idx ON sessions(workspace)",
    // `ORDER BY started_at_ms` for a workspace (list_sessions, recent_sessions_3)
    "CREATE INDEX IF NOT EXISTS sessions_workspace_started_idx ON sessions(workspace, started_at_ms)",
    "CREATE INDEX IF NOT EXISTS sessions_workspace_started_desc_idx
        ON sessions(workspace, started_at_ms DESC, id ASC)",
    "CREATE INDEX IF NOT EXISTS sessions_workspace_agent_lower_idx
        ON sessions(workspace, lower(agent), started_at_ms DESC, id ASC)",
    "CREATE TABLE IF NOT EXISTS rules_used (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        rule TEXT NOT NULL
    )",
    "CREATE UNIQUE INDEX IF NOT EXISTS rules_used_session_rule_idx ON rules_used(session_id, rule)",
    // Provider pull cache (single-row state + per-kind rows; atomic refresh = txn + clear + insert)
    "CREATE TABLE IF NOT EXISTS remote_pull_state (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        query_provider TEXT NOT NULL DEFAULT 'none',
        cursor_json TEXT NOT NULL DEFAULT '',
        last_success_ms INTEGER
    )",
    "INSERT OR IGNORE INTO remote_pull_state (id) VALUES (1)",
    "CREATE TABLE IF NOT EXISTS remote_sessions (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        session_id_hash TEXT NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, session_id_hash)
    )",
    "CREATE TABLE IF NOT EXISTS remote_events (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        session_id_hash TEXT NOT NULL,
        event_seq INTEGER NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, session_id_hash, event_seq)
    )",
    "CREATE TABLE IF NOT EXISTS remote_tool_spans (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        span_id_hash TEXT NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, span_id_hash)
    )",
    "CREATE TABLE IF NOT EXISTS remote_repo_snapshots (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        snapshot_id_hash TEXT NOT NULL,
        chunk_index INTEGER NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, snapshot_id_hash, chunk_index)
    )",
    "CREATE TABLE IF NOT EXISTS remote_workspace_facts (
        team_id TEXT NOT NULL,
        workspace_hash TEXT NOT NULL,
        fact_key TEXT NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (team_id, workspace_hash, fact_key)
    )",
    "CREATE TABLE IF NOT EXISTS session_evals (
        id            TEXT    PRIMARY KEY,
        session_id    TEXT    NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        judge_model   TEXT    NOT NULL,
        rubric_id     TEXT    NOT NULL,
        score         REAL    NOT NULL CHECK(score BETWEEN 0.0 AND 1.0),
        rationale     TEXT    NOT NULL,
        flagged       INTEGER NOT NULL DEFAULT 0,
        created_at_ms INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS session_evals_session ON session_evals(session_id);
    CREATE INDEX IF NOT EXISTS session_evals_rubric  ON session_evals(rubric_id, score)",
    "CREATE TABLE IF NOT EXISTS prompt_snapshots (
        fingerprint   TEXT    PRIMARY KEY,
        captured_at_ms INTEGER NOT NULL,
        files_json    TEXT    NOT NULL,
        total_bytes   INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS session_feedback (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        score INTEGER CHECK(score BETWEEN 1 AND 5),
        label TEXT CHECK(label IN ('good','bad','interesting','bug','regression')),
        note TEXT,
        created_at_ms INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS session_feedback_session ON session_feedback(session_id);
    CREATE INDEX IF NOT EXISTS session_feedback_label ON session_feedback(label, created_at_ms)",
    "CREATE TABLE IF NOT EXISTS session_outcomes (
        session_id TEXT PRIMARY KEY NOT NULL,
        test_passed INTEGER,
        test_failed INTEGER,
        test_skipped INTEGER,
        build_ok INTEGER,
        lint_errors INTEGER,
        revert_lines_14d INTEGER,
        pr_open INTEGER,
        ci_ok INTEGER,
        measured_at_ms INTEGER NOT NULL,
        measure_error TEXT
    )",
    "CREATE TABLE IF NOT EXISTS session_samples (
        session_id TEXT NOT NULL,
        ts_ms INTEGER NOT NULL,
        pid INTEGER NOT NULL,
        cpu_percent REAL,
        rss_bytes INTEGER,
        PRIMARY KEY (session_id, ts_ms, pid)
    )",
    "CREATE TABLE IF NOT EXISTS cases (
        id TEXT PRIMARY KEY,
        source_key TEXT NOT NULL UNIQUE,
        session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        reason TEXT NOT NULL,
        label TEXT,
        status TEXT NOT NULL CHECK(status IN ('open','archived')),
        prompt_fingerprint TEXT,
        metadata_json TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS case_refs (
        case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
        ref_kind TEXT NOT NULL,
        ref_key TEXT NOT NULL,
        PRIMARY KEY (case_id, ref_kind, ref_key)
    )",
    "CREATE TABLE IF NOT EXISTS rules (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        filter TEXT NOT NULL,
        action_json TEXT NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1,
        created_at_ms INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS review_items (
        id TEXT PRIMARY KEY,
        source_key TEXT NOT NULL UNIQUE,
        session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        title TEXT NOT NULL,
        status TEXT NOT NULL CHECK(status IN ('open','resolved','dismissed')),
        created_at_ms INTEGER NOT NULL,
        resolved_at_ms INTEGER
    )",
    "CREATE TABLE IF NOT EXISTS alert_events (
        id TEXT PRIMARY KEY,
        source_key TEXT NOT NULL UNIQUE,
        name TEXT NOT NULL,
        severity TEXT NOT NULL CHECK(severity IN ('info','warning','critical')),
        message TEXT NOT NULL,
        session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
        created_at_ms INTEGER NOT NULL
    )",
    "CREATE INDEX IF NOT EXISTS session_samples_session_idx ON session_samples(session_id)",
    "CREATE INDEX IF NOT EXISTS cases_status_idx ON cases(status, created_at_ms)",
    "CREATE INDEX IF NOT EXISTS review_items_status_idx ON review_items(status, created_at_ms)",
    "CREATE INDEX IF NOT EXISTS alert_events_name_idx ON alert_events(name, created_at_ms)",
    "CREATE INDEX IF NOT EXISTS tool_spans_session_idx ON tool_spans(session_id)",
    "CREATE INDEX IF NOT EXISTS tool_spans_started_idx ON tool_spans(started_at_ms)",
    "CREATE INDEX IF NOT EXISTS tool_spans_ended_idx ON tool_spans(ended_at_ms)",
    "CREATE INDEX IF NOT EXISTS session_samples_ts_idx ON session_samples(ts_ms)",
    "CREATE INDEX IF NOT EXISTS events_ts_idx ON events(ts_ms)",
    "CREATE INDEX IF NOT EXISTS events_ts_session_seq_idx ON events(ts_ms, session_id, seq)",
    "CREATE INDEX IF NOT EXISTS events_session_ts_seq_idx ON events(session_id, ts_ms, seq)",
    "CREATE INDEX IF NOT EXISTS events_tool_ts_session_seq_idx ON events(tool, ts_ms DESC, session_id, seq)",
    "CREATE INDEX IF NOT EXISTS tool_spans_session_started_idx ON tool_spans(session_id, started_at_ms)",
    "CREATE INDEX IF NOT EXISTS tool_spans_session_ended_idx ON tool_spans(session_id, ended_at_ms)",
    "CREATE INDEX IF NOT EXISTS tool_span_paths_path_idx ON tool_span_paths(path, span_id)",
    "CREATE INDEX IF NOT EXISTS feedback_session_idx ON session_feedback(session_id)",
    "CREATE TABLE IF NOT EXISTS guidance_candidates (
        id TEXT PRIMARY KEY,
        artifact_kind TEXT NOT NULL CHECK(artifact_kind IN ('skill','rule')),
        artifact_id TEXT NOT NULL,
        action_json TEXT NOT NULL,
        status TEXT NOT NULL CHECK(status IN ('proposed','applied','validated','rejected','archived')),
        rationale TEXT NOT NULL,
        evidence_json TEXT NOT NULL,
        created_at_ms INTEGER NOT NULL,
        applied_at_ms INTEGER,
        treatment_fingerprint TEXT,
        experiment_id TEXT,
        backup_path TEXT
    );
    CREATE INDEX IF NOT EXISTS guidance_candidates_status_idx
        ON guidance_candidates(status, created_at_ms)",
];

pub(super) fn mmap_size_bytes_from_mb(raw: Option<&str>) -> i64 {
    raw.and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_MMAP_MB)
        .saturating_mul(1024)
        .saturating_mul(1024)
        .min(i64::MAX as u64) as i64
}

pub(super) fn apply_pragmas(conn: &Connection, mode: StoreOpenMode) -> Result<()> {
    let mmap_size = mmap_size_bytes_from_mb(std::env::var("KAIZEN_MMAP_MB").ok().as_deref());
    conn.execute_batch(&format!(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA busy_timeout=5000;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-65536;
        PRAGMA mmap_size={mmap_size};
        PRAGMA temp_store=MEMORY;
        PRAGMA wal_autocheckpoint=1000;
        "
    ))?;
    if mode == StoreOpenMode::ReadOnlyQuery {
        conn.execute_batch("PRAGMA query_only=ON;")?;
    }
    Ok(())
}

pub(super) fn ensure_schema_columns(conn: &Connection) -> Result<()> {
    ensure_column(conn, "sessions", "start_commit", "TEXT")?;
    ensure_column(conn, "sessions", "end_commit", "TEXT")?;
    ensure_column(conn, "sessions", "branch", "TEXT")?;
    ensure_column(conn, "sessions", "dirty_start", "INTEGER")?;
    ensure_column(conn, "sessions", "dirty_end", "INTEGER")?;
    ensure_column(
        conn,
        "sessions",
        "repo_binding_source",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(conn, "events", "ts_exact", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "events", "tool_call_id", "TEXT")?;
    ensure_column(conn, "events", "reasoning_tokens", "INTEGER")?;
    ensure_column(conn, "events", "stop_reason", "TEXT")?;
    ensure_column(conn, "events", "latency_ms", "INTEGER")?;
    ensure_column(conn, "events", "ttft_ms", "INTEGER")?;
    ensure_column(conn, "events", "retry_count", "INTEGER")?;
    ensure_column(conn, "events", "context_used_tokens", "INTEGER")?;
    ensure_column(conn, "events", "context_max_tokens", "INTEGER")?;
    ensure_column(conn, "events", "cache_creation_tokens", "INTEGER")?;
    ensure_column(conn, "events", "cache_read_tokens", "INTEGER")?;
    ensure_column(conn, "events", "system_prompt_tokens", "INTEGER")?;
    ensure_column(
        conn,
        "sync_outbox",
        "kind",
        "TEXT NOT NULL DEFAULT 'events'",
    )?;
    ensure_column(
        conn,
        "experiments",
        "state",
        "TEXT NOT NULL DEFAULT 'Draft'",
    )?;
    ensure_column(conn, "experiments", "concluded_at_ms", "INTEGER")?;
    ensure_column(conn, "sessions", "prompt_fingerprint", "TEXT")?;
    ensure_column(conn, "sessions", "parent_session_id", "TEXT")?;
    ensure_column(conn, "sessions", "agent_version", "TEXT")?;
    ensure_column(conn, "sessions", "os", "TEXT")?;
    ensure_column(conn, "sessions", "arch", "TEXT")?;
    ensure_column(conn, "sessions", "repo_file_count", "INTEGER")?;
    ensure_column(conn, "sessions", "repo_total_loc", "INTEGER")?;
    ensure_column(conn, "tool_spans", "parent_span_id", "TEXT")?;
    ensure_column(conn, "tool_spans", "depth", "INTEGER NOT NULL DEFAULT 0")?;
    ensure_column(conn, "tool_spans", "subtree_cost_usd_e6", "INTEGER")?;
    ensure_column(conn, "tool_spans", "subtree_token_count", "INTEGER")?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS tool_spans_parent ON tool_spans(parent_span_id);
         CREATE INDEX IF NOT EXISTS tool_spans_session_depth ON tool_spans(session_id, depth);",
    )?;
    Ok(())
}

pub(super) fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    sql_type: &str,
) -> Result<()> {
    if has_column(conn, table, column)? {
        return Ok(());
    }
    let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {sql_type}");
    match conn.execute(&sql, []) {
        Ok(_) => Ok(()),
        Err(err) if column_was_added_by_race(conn, table, column, &err)? => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub(super) fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.filter_map(|r| r.ok()).any(|name| name == column))
}

pub(super) fn column_was_added_by_race(
    conn: &Connection,
    table: &str,
    column: &str,
    err: &rusqlite::Error,
) -> Result<bool> {
    if !is_duplicate_column_error(err) {
        return Ok(false);
    }
    has_column(conn, table, column)
}

pub(super) fn is_duplicate_column_error(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message.contains("duplicate column name")
    )
}
