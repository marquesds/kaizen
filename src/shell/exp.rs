// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen exp` — experiment CRUD + report rendering.

use crate::core::config;
use crate::core::event::{Event, SessionRecord};
use crate::core::repo::repo_head;
use crate::experiment::store as exp_store;
use crate::experiment::types::{
    Binding, Classification, Criterion, Direction, Experiment, Metric, State, transition,
};
use crate::experiment::{self as exp};
use crate::shell::cli::{scan_all_agents, workspace_path};
use crate::store::Store;
use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct NewArgs {
    pub name: String,
    pub hypothesis: String,
    pub change: String,
    pub metric: String,
    pub bind: String,
    pub duration_days: u32,
    pub target_pct: f64,
    pub control_commit: Option<String>,
    pub treatment_commit: Option<String>,
}

pub fn exp_new_text(workspace: Option<&Path>, args: NewArgs) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let db_path = ws.join(".kaizen/kaizen.db");
    let store = Store::open(&db_path)?;
    let metric =
        Metric::parse(&args.metric).ok_or_else(|| anyhow!("unknown metric: {}", args.metric))?;
    let binding = build_binding(&ws, &args)?;
    let (direction, target_pct) = split_target(args.target_pct);
    let exp_rec = Experiment {
        id: uuid::Uuid::now_v7().to_string(),
        name: args.name.clone(),
        hypothesis: args.hypothesis,
        change_description: args.change,
        metric,
        binding,
        duration_days: args.duration_days,
        success_criterion: Criterion::Delta {
            direction,
            target_pct,
        },
        state: State::Running,
        created_at_ms: now_ms(),
        concluded_at_ms: None,
    };
    exp_store::save_experiment(&store, &exp_rec)?;
    Ok(format!("created {} · {}\n", exp_rec.id, exp_rec.name))
}

pub fn cmd_new(workspace: Option<&Path>, args: NewArgs) -> Result<()> {
    print!("{}", exp_new_text(workspace, args)?);
    Ok(())
}

fn build_binding(ws: &Path, args: &NewArgs) -> Result<Binding> {
    match args.bind.as_str() {
        "git" => {
            let treatment = match args.treatment_commit.clone() {
                Some(v) => v,
                None => repo_head(ws)?
                    .ok_or_else(|| anyhow!("not a git repo; pass --treatment-commit"))?,
            };
            let control = match args.control_commit.clone() {
                Some(v) => v,
                None => parent_of(ws, &treatment)?,
            };
            Ok(Binding::GitCommit {
                control_commit: control,
                treatment_commit: treatment,
            })
        }
        "manual" => Ok(Binding::ManualTag {
            variant_field: "variant".into(),
        }),
        other => Err(anyhow!("unsupported bind: {other} (use git|manual)")),
    }
}

fn split_target(pct: f64) -> (Direction, f64) {
    if pct < 0.0 {
        (Direction::Decrease, pct)
    } else {
        (Direction::Increase, pct)
    }
}

fn parent_of(ws: &Path, commit: &str) -> Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(ws)
        .args(["rev-parse", &format!("{commit}^")])
        .output()
        .context("git rev-parse parent")?;
    if !out.status.success() {
        return Err(anyhow!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn exp_list_text(workspace: Option<&Path>) -> Result<String> {
    use std::fmt::Write;
    let ws = workspace_path(workspace)?;
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let all = exp_store::list_experiments(&store)?;
    let mut out = String::new();
    if all.is_empty() {
        writeln!(&mut out, "(no experiments)").unwrap();
        return Ok(out);
    }
    writeln!(
        &mut out,
        "{:<38} {:<10} {:<24} METRIC",
        "ID", "STATE", "NAME"
    )
    .unwrap();
    writeln!(&mut out, "{}", "-".repeat(96)).unwrap();
    for e in &all {
        writeln!(
            &mut out,
            "{:<38} {:<10?} {:<24} {}",
            e.id,
            e.state,
            truncate(&e.name, 24),
            e.metric.as_str()
        )
        .unwrap();
    }
    Ok(out)
}

pub fn cmd_list(workspace: Option<&Path>) -> Result<()> {
    print!("{}", exp_list_text(workspace)?);
    Ok(())
}

pub fn exp_status_text(workspace: Option<&Path>, id: &str) -> Result<String> {
    use std::fmt::Write;
    let ws = workspace_path(workspace)?;
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let e = exp_store::load_experiment(&store, id)?
        .ok_or_else(|| anyhow!("experiment not found: {id}"))?;
    let mut out = String::new();
    writeln!(&mut out, "id:         {}", e.id).unwrap();
    writeln!(&mut out, "name:       {}", e.name).unwrap();
    writeln!(&mut out, "state:      {:?}", e.state).unwrap();
    writeln!(&mut out, "metric:     {}", e.metric.as_str()).unwrap();
    writeln!(&mut out, "duration:   {}d", e.duration_days).unwrap();
    writeln!(&mut out, "created:    {}", e.created_at_ms).unwrap();
    if let Some(c) = e.concluded_at_ms {
        writeln!(&mut out, "concluded:  {c}").unwrap();
    }
    writeln!(&mut out, "hypothesis: {}", e.hypothesis).unwrap();
    writeln!(&mut out, "change:     {}", e.change_description).unwrap();
    match &e.binding {
        Binding::GitCommit {
            control_commit,
            treatment_commit,
        } => {
            writeln!(
                &mut out,
                "binding:    git control={control_commit} treatment={treatment_commit}"
            )
            .unwrap();
        }
        Binding::Branch {
            control_branch,
            treatment_branch,
        } => {
            writeln!(
                &mut out,
                "binding:    branch control={control_branch} treatment={treatment_branch}"
            )
            .unwrap();
        }
        Binding::ManualTag { variant_field } => {
            writeln!(&mut out, "binding:    manual({variant_field})").unwrap();
        }
    }
    Ok(out)
}

pub fn cmd_status(workspace: Option<&Path>, id: &str) -> Result<()> {
    print!("{}", exp_status_text(workspace, id)?);
    Ok(())
}

pub fn exp_tag_text(
    workspace: Option<&Path>,
    id: &str,
    session_id: &str,
    variant: &str,
) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let v = match variant {
        "control" => Classification::Control,
        "treatment" => Classification::Treatment,
        "excluded" => Classification::Excluded,
        other => {
            return Err(anyhow!(
                "variant must be control|treatment|excluded, got {other}"
            ));
        }
    };
    exp_store::tag_session(&store, id, session_id, v)?;
    Ok(format!("tagged {session_id} -> {variant} for {id}\n"))
}

pub fn cmd_tag(workspace: Option<&Path>, id: &str, session_id: &str, variant: &str) -> Result<()> {
    print!("{}", exp_tag_text(workspace, id, session_id, variant)?);
    Ok(())
}

pub fn exp_report_text(workspace: Option<&Path>, id: &str, json_out: bool) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let ws_str = ws.to_string_lossy().to_string();
    scan_all_agents(&ws, &cfg, &ws_str, &store)?;
    let exp_rec = exp_store::load_experiment(&store, id)?
        .ok_or_else(|| anyhow!("experiment not found: {id}"))?;
    let (start_ms, end_ms) = window_for(&exp_rec);
    let sessions = sessions_with_events_in(&store, &ws_str, start_ms, end_ms)?;
    let manual = exp_store::manual_tags(&store, id)?;
    let report = exp::run(&exp_rec, &sessions, &manual, &ws);
    if json_out {
        Ok(serde_json::to_string_pretty(&report)?)
    } else {
        Ok(exp::to_markdown(&report))
    }
}

pub fn cmd_report(workspace: Option<&Path>, id: &str, json_out: bool) -> Result<()> {
    print!("{}", exp_report_text(workspace, id, json_out)?);
    Ok(())
}

pub fn exp_conclude_text(workspace: Option<&Path>, id: &str) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&ws.join(".kaizen/kaizen.db"))?;
    let exp_rec = exp_store::load_experiment(&store, id)?
        .ok_or_else(|| anyhow!("experiment not found: {id}"))?;
    let next = transition(exp_rec.state, "conclude")
        .ok_or_else(|| anyhow!("cannot conclude from {:?}", exp_rec.state))?;
    exp_store::set_state(&store, id, next, now_ms())?;
    Ok(format!("concluded {id}\n"))
}

pub fn cmd_conclude(workspace: Option<&Path>, id: &str) -> Result<()> {
    print!("{}", exp_conclude_text(workspace, id)?);
    Ok(())
}

fn window_for(e: &Experiment) -> (u64, u64) {
    let end = e
        .concluded_at_ms
        .unwrap_or_else(|| e.created_at_ms + (e.duration_days as u64) * 86_400_000);
    (e.created_at_ms, end.max(e.created_at_ms))
}

fn sessions_with_events_in(
    store: &Store,
    ws: &str,
    start_ms: u64,
    end_ms: u64,
) -> Result<Vec<(SessionRecord, Vec<Event>)>> {
    let rows = store.retro_events_in_window(ws, start_ms, end_ms)?;
    let mut by_id: std::collections::BTreeMap<String, (SessionRecord, Vec<Event>)> =
        std::collections::BTreeMap::new();
    for (s, e) in rows {
        by_id
            .entry(s.id.clone())
            .or_insert_with(|| (s.clone(), Vec::new()))
            .1
            .push(e);
    }
    Ok(by_id.into_values().collect())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
