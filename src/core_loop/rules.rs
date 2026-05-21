// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{AlertSeverity, LocalRule, RuleAction, TraceHit};
use crate::store::Store;
use anyhow::{Result, anyhow};
use rusqlite::{OptionalExtension, params};

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleRun {
    pub rule_id: String,
    pub hits: usize,
    pub actions: usize,
}

pub fn create(
    store: &Store,
    name: &str,
    filter: &str,
    action: RuleAction,
    now_ms: u64,
) -> Result<LocalRule> {
    let rule = LocalRule {
        id: uuid::Uuid::now_v7().to_string(),
        name: name.into(),
        filter: filter.into(),
        action,
        enabled: true,
        created_at_ms: now_ms,
    };
    store.conn().execute(
        "INSERT INTO rules (id, name, filter, action_json, enabled, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        params![
            rule.id,
            rule.name,
            rule.filter,
            serde_json::to_string(&rule.action)?,
            rule.created_at_ms as i64
        ],
    )?;
    Ok(rule)
}

pub fn list(store: &Store) -> Result<Vec<LocalRule>> {
    let mut stmt = store.conn().prepare("SELECT id, name, filter, action_json, enabled, created_at_ms FROM rules ORDER BY created_at_ms DESC")?;
    let rows = stmt.query_map([], row)?;
    rows.map(|r| r.map_err(anyhow::Error::from)).collect()
}

pub fn set_enabled(store: &Store, id: &str, enabled: bool) -> Result<()> {
    store.conn().execute(
        "UPDATE rules SET enabled = ?2 WHERE id = ?1",
        params![id, enabled as i64],
    )?;
    Ok(())
}

pub fn run_enabled(
    store: &Store,
    workspace: &str,
    start_ms: u64,
    now_ms: u64,
    dry_run: bool,
) -> Result<Vec<RuleRun>> {
    list(store)?
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| run_one(store, workspace, start_ms, now_ms, dry_run, r))
        .collect()
}

fn run_one(
    store: &Store,
    workspace: &str,
    start_ms: u64,
    now_ms: u64,
    dry_run: bool,
    rule: LocalRule,
) -> Result<RuleRun> {
    let hits = crate::core_loop::query::run(store, workspace, &rule.filter, start_ms, 100)?;
    let actions = if dry_run {
        0
    } else {
        apply_all(store, &rule, &hits, now_ms)?
    };
    Ok(RuleRun {
        rule_id: rule.id,
        hits: hits.len(),
        actions,
    })
}

fn apply_all(store: &Store, rule: &LocalRule, hits: &[TraceHit], now_ms: u64) -> Result<usize> {
    hits.iter()
        .map(|h| apply_one(store, rule, h, now_ms))
        .try_fold(0, |n, r| r.map(|_| n + 1))
}

fn apply_one(store: &Store, rule: &LocalRule, hit: &TraceHit, now_ms: u64) -> Result<()> {
    match &rule.action {
        RuleAction::CreateCase { label } => case_action(store, rule, hit, label.clone(), now_ms),
        RuleAction::QueueReview { title } => review_action(store, rule, hit, title.clone(), now_ms),
        RuleAction::EmitAlert { severity } => alert_action(store, rule, hit, *severity, now_ms),
    }
}

fn case_action(
    store: &Store,
    rule: &LocalRule,
    hit: &TraceHit,
    label: Option<String>,
    now_ms: u64,
) -> Result<()> {
    let s = store
        .get_session(&hit.session_id)?
        .ok_or_else(|| anyhow!("session not found"))?;
    let key = format!("rule:{}:case:{}", rule.id, hit_key(hit));
    let rec = crate::core_loop::cases::create_case(
        store,
        &s,
        &key,
        &format!("rule:{}", rule.name),
        label,
        now_ms,
    )?;
    crate::core_loop::cases::add_ref(store, &rec.id, "hit", &hit_key(hit))
}

fn review_action(
    store: &Store,
    rule: &LocalRule,
    hit: &TraceHit,
    title: Option<String>,
    now_ms: u64,
) -> Result<()> {
    let title = title.unwrap_or_else(|| format!("Review {}", rule.name));
    let key = format!("rule:{}:review:{}", rule.id, hit_key(hit));
    crate::core_loop::review::create(store, &key, &hit.session_id, &title, now_ms)?;
    Ok(())
}

fn alert_action(
    store: &Store,
    rule: &LocalRule,
    hit: &TraceHit,
    severity: AlertSeverity,
    now_ms: u64,
) -> Result<()> {
    let key = format!("rule:{}:alert:{}", rule.id, hit_key(hit));
    crate::core_loop::alerts::emit(
        store,
        &key,
        &rule.name,
        severity,
        &hit.summary,
        Some(&hit.session_id),
        now_ms,
    )?;
    Ok(())
}

fn hit_key(hit: &TraceHit) -> String {
    hit.seq
        .map(|s| format!("{}:{s}", hit.session_id))
        .unwrap_or_else(|| hit.session_id.clone())
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LocalRule> {
    let action_json: String = r.get(3)?;
    Ok(LocalRule {
        id: r.get(0)?,
        name: r.get(1)?,
        filter: r.get(2)?,
        action: serde_json::from_str(&action_json).unwrap_or(RuleAction::EmitAlert {
            severity: AlertSeverity::Warning,
        }),
        enabled: r.get::<_, i64>(4)? != 0,
        created_at_ms: r.get::<_, i64>(5)? as u64,
    })
}

pub fn get(store: &Store, id: &str) -> Result<LocalRule> {
    let sql =
        "SELECT id, name, filter, action_json, enabled, created_at_ms FROM rules WHERE id = ?1";
    store
        .conn()
        .query_row(sql, params![id], row)
        .optional()?
        .ok_or_else(|| anyhow!("rule not found: {id}"))
}
