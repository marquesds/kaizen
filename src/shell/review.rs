// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::core_loop::{ReviewStatus, review, time};
use crate::shell::cli::workspace_path;
use crate::store::Store;
use anyhow::Result;
use std::path::Path;

pub fn cmd_review_list(workspace: Option<&Path>, status: Option<String>, json: bool) -> Result<()> {
    output(
        &review::list(&open(workspace)?, parse_status(status.as_deref()))?,
        json,
    )
}

pub fn cmd_review_show(workspace: Option<&Path>, id: &str, json: bool) -> Result<()> {
    let row = review::get(&open(workspace)?, id)?;
    if json {
        println!("{}", json_one(&row)?);
        return Ok(());
    }
    output(&[row], false)
}

pub fn cmd_review_resolve(workspace: Option<&Path>, id: &str) -> Result<()> {
    set(workspace, id, ReviewStatus::Resolved, "resolved")
}

pub fn cmd_review_dismiss(workspace: Option<&Path>, id: &str) -> Result<()> {
    set(workspace, id, ReviewStatus::Dismissed, "dismissed")
}

fn set(workspace: Option<&Path>, id: &str, status: ReviewStatus, label: &str) -> Result<()> {
    review::set_status(&open(workspace)?, id, status, time::now_ms())?;
    println!("{label} review {id}");
    Ok(())
}

fn open(workspace: Option<&Path>) -> Result<Store> {
    let ws = workspace_path(workspace)?;
    Store::open(&crate::core::workspace::db_path(&ws)?)
}

fn output(rows: &[crate::core_loop::ReviewItem], json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(rows)?);
    } else {
        rows.iter().for_each(|r| {
            println!(
                "{} session={} status={} {}",
                r.id,
                r.session_id,
                r.status.as_str(),
                r.title
            )
        });
    }
    Ok(())
}

fn json_one(row: &crate::core_loop::ReviewItem) -> Result<String> {
    Ok(serde_json::to_string_pretty(row)?)
}

fn parse_status(raw: Option<&str>) -> Option<ReviewStatus> {
    raw.map(|s| match s {
        "resolved" => ReviewStatus::Resolved,
        "dismissed" => ReviewStatus::Dismissed,
        _ => ReviewStatus::Open,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_loop::ReviewItem;

    #[test]
    fn review_show_json_renders_single_object() {
        let text = json_one(&item()).unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text)
                .unwrap()
                .is_object()
        );
    }

    fn item() -> ReviewItem {
        ReviewItem {
            id: "r1".into(),
            source_key: "k".into(),
            session_id: "s1".into(),
            title: "Review".into(),
            status: ReviewStatus::Open,
            created_at_ms: 1,
            resolved_at_ms: None,
        }
    }
}
