// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{AlertSeverity, RuleAction, rules, time};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::{Result, anyhow};
use std::path::Path;

pub fn cmd_rules_create(
    workspace: Option<&Path>,
    name: &str,
    filter: &str,
    action: &str,
    message: Option<String>,
) -> Result<()> {
    let rule = rules::create(
        &open(workspace)?,
        name,
        filter,
        parse_action(action, message)?,
        time::now_ms(),
    )?;
    println!("created rule {} · {}", rule.id, rule.name);
    Ok(())
}

pub fn cmd_rules_list(workspace: Option<&Path>, json: bool) -> Result<()> {
    let rows = rules::list(&open(workspace)?)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        rows.iter()
            .for_each(|r| println!("{} enabled={} {}", r.id, r.enabled, r.name));
    }
    Ok(())
}

pub fn cmd_rules_run(
    workspace: Option<&Path>,
    since: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let rows = rules::run_enabled(
        &store,
        &ws.to_string_lossy(),
        time::parse_window(since, 7)?,
        time::now_ms(),
        dry_run,
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        rows.iter()
            .for_each(|r| println!("{} hits={} actions={}", r.rule_id, r.hits, r.actions));
    }
    Ok(())
}

pub fn cmd_rules_enable(workspace: Option<&Path>, id: &str, enabled: bool) -> Result<()> {
    rules::set_enabled(&open(workspace)?, id, enabled)?;
    println!("{} rule {id}", if enabled { "enabled" } else { "disabled" });
    Ok(())
}

fn open(workspace: Option<&Path>) -> Result<Store> {
    let ws = workspace_path(workspace)?;
    Store::open(&crate::core::workspace::db_path(&ws)?)
}

fn parse_action(raw: &str, message: Option<String>) -> Result<RuleAction> {
    Ok(match raw {
        "create_case" => RuleAction::CreateCase { label: message },
        "queue_review" => RuleAction::QueueReview { title: message },
        "emit_alert" => RuleAction::EmitAlert {
            severity: AlertSeverity::Warning,
        },
        _ => {
            return Err(anyhow!(
                "action must be create_case, queue_review, or emit_alert"
            ));
        }
    })
}
