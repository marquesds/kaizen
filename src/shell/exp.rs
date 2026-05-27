// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen exp` — experiment CRUD + report rendering.

use crate::core::config;
use crate::core::repo::repo_head;
use crate::experiment::store as exp_store;
use crate::experiment::types::{
    Binding, Classification, Criterion, Direction, Experiment, Metric, State, transition,
};
use crate::experiment::{self as exp};
use crate::shell::cli::{maybe_scan_all_agents, open_workspace_read_store, workspace_path};
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
    pub control_branch: Option<String>,
    pub treatment_branch: Option<String>,
    pub control_fingerprint: Option<String>,
    pub treatment_fingerprint: Option<String>,
}

pub fn exp_new_text(workspace: Option<&Path>, args: NewArgs) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let metric =
        Metric::parse(&args.metric).ok_or_else(|| anyhow!("unknown metric: {}", args.metric))?;
    let binding = build_binding(&ws, &args)?;
    let (direction, target_pct) = split_target(args.target_pct);
    let created_at = now_ms();
    let exp_rec = Experiment {
        id: deterministic_exp_id(&args.name, created_at),
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
        state: State::Draft,
        created_at_ms: created_at,
        concluded_at_ms: None,
        guardrails: Vec::new(),
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
        "branch" => {
            let control = args
                .control_branch
                .clone()
                .ok_or_else(|| anyhow!("--control-branch required for --bind branch"))?;
            let treatment = args
                .treatment_branch
                .clone()
                .ok_or_else(|| anyhow!("--treatment-branch required for --bind branch"))?;
            Ok(Binding::Branch {
                control_branch: control,
                treatment_branch: treatment,
            })
        }
        "manual" => Ok(Binding::ManualTag {
            variant_field: "variant".into(),
        }),
        "prompt" => Ok(Binding::PromptFingerprint {
            control_fingerprint: args
                .control_fingerprint
                .clone()
                .ok_or_else(|| anyhow!("--control-fingerprint required for --bind prompt"))?,
            treatment_fingerprint: args
                .treatment_fingerprint
                .clone()
                .ok_or_else(|| anyhow!("--treatment-fingerprint required for --bind prompt"))?,
        }),
        other => Err(anyhow!(
            "unsupported bind: {other} (use git|branch|manual|prompt)"
        )),
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
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
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
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
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
        Binding::PromptFingerprint {
            control_fingerprint,
            treatment_fingerprint,
        } => {
            writeln!(
                &mut out,
                "binding:    prompt control={control_fingerprint} treatment={treatment_fingerprint}"
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
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
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

pub fn exp_report_text(
    workspace: Option<&Path>,
    id: &str,
    json_out: bool,
    refresh: bool,
) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = open_workspace_read_store(&ws, refresh)?;
    let ws_str = ws.to_string_lossy().to_string();
    if refresh {
        let cfg = config::load(&ws)?;
        maybe_scan_all_agents(&ws, &cfg, &ws_str, &store, true)?;
    }
    let exp_rec = exp_store::load_experiment(&store, id)?
        .ok_or_else(|| anyhow!("experiment not found: {id}"))?;
    let (start_ms, end_ms) = window_for(&exp_rec);
    let manual = exp_store::manual_tags(&store, id)?;
    let (sessions, values) =
        metric_values_by_session(&store, &ws_str, start_ms, end_ms, exp_rec.metric)?;
    let mut guardrail_values = std::collections::HashMap::new();
    for guardrail in &exp_rec.guardrails {
        let (_, values) =
            metric_values_by_session(&store, &ws_str, start_ms, end_ms, guardrail.metric)?;
        guardrail_values.insert(guardrail.metric, values);
    }
    let report = exp::run_from_metric_values(
        &exp_rec,
        &sessions,
        &values,
        &guardrail_values,
        &manual,
        &ws,
        false,
    );
    if json_out {
        Ok(serde_json::to_string_pretty(&report)?)
    } else {
        Ok(exp::to_markdown(&report))
    }
}

pub fn cmd_report(workspace: Option<&Path>, id: &str, json_out: bool, refresh: bool) -> Result<()> {
    print!("{}", exp_report_text(workspace, id, json_out, refresh)?);
    Ok(())
}

pub fn exp_conclude_text(workspace: Option<&Path>, id: &str) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
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

pub fn exp_start_text(workspace: Option<&Path>, id: &str) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let exp_rec = exp_store::load_experiment(&store, id)?
        .ok_or_else(|| anyhow!("experiment not found: {id}"))?;
    let next = transition(exp_rec.state, "start")
        .ok_or_else(|| anyhow!("cannot start from {:?}", exp_rec.state))?;
    exp_store::set_state(&store, id, next, now_ms())?;
    Ok(format!("started {id}\n"))
}

pub fn cmd_start(workspace: Option<&Path>, id: &str) -> Result<()> {
    print!("{}", exp_start_text(workspace, id)?);
    Ok(())
}

pub fn exp_archive_text(workspace: Option<&Path>, id: &str) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let store = Store::open(&crate::core::workspace::db_path(&ws)?)?;
    let exp_rec = exp_store::load_experiment(&store, id)?
        .ok_or_else(|| anyhow!("experiment not found: {id}"))?;
    let next = transition(exp_rec.state, "archive")
        .ok_or_else(|| anyhow!("cannot archive from {:?}", exp_rec.state))?;
    exp_store::set_state(&store, id, next, now_ms())?;
    Ok(format!("archived {id}\n"))
}

pub fn cmd_archive(workspace: Option<&Path>, id: &str) -> Result<()> {
    print!("{}", exp_archive_text(workspace, id)?);
    Ok(())
}

pub fn exp_power_text(
    workspace: Option<&Path>,
    metric: &str,
    baseline_n: usize,
    refresh: bool,
) -> Result<String> {
    use crate::experiment::stats::power;
    use std::fmt::Write;

    let ws = workspace_path(workspace)?;
    let store = open_workspace_read_store(&ws, refresh)?;
    let ws_str = ws.to_string_lossy().to_string();
    if refresh {
        let cfg = config::load(&ws)?;
        maybe_scan_all_agents(&ws, &cfg, &ws_str, &store, true)?;
    }

    let metric_val = Metric::parse(metric).ok_or_else(|| anyhow!("unknown metric: {metric}"))?;
    let now = now_ms();
    let lookback_ms = 90 * 86_400_000_u64;
    let values = store
        .experiment_metric_values_in_window(
            &ws_str,
            now.saturating_sub(lookback_ms),
            now,
            metric_val,
        )?
        .into_iter()
        .map(|(_, value)| value)
        .collect::<Vec<_>>();

    let mut out = String::new();
    match power::mde(&values, baseline_n) {
        None => writeln!(&mut out, "no data for metric {metric} in the last 90 days").unwrap(),
        Some(r) => {
            writeln!(&mut out, "metric:      {metric}").unwrap();
            writeln!(&mut out, "baseline n:  {}", r.n_per_arm).unwrap();
            writeln!(&mut out, "observed σ:  {:.3}", r.sigma).unwrap();
            writeln!(&mut out, "MDE:         {:.3}", r.mde_absolute).unwrap();
            if let Some(pct) = r.mde_pct {
                writeln!(&mut out, "MDE %:       {:.1}%", pct).unwrap();
            }
            writeln!(
                &mut out,
                "\n(80% power · 95% CI · {n} sessions in baseline)",
                n = values.len()
            )
            .unwrap();
        }
    }
    Ok(out)
}

fn metric_values_by_session(
    store: &Store,
    ws: &str,
    start_ms: u64,
    end_ms: u64,
    metric: Metric,
) -> Result<(
    Vec<crate::core::event::SessionRecord>,
    std::collections::HashMap<String, f64>,
)> {
    let rows = store.experiment_metric_values_in_window(ws, start_ms, end_ms, metric)?;
    let mut sessions = Vec::with_capacity(rows.len());
    let mut values = std::collections::HashMap::with_capacity(rows.len());
    for (session, value) in rows {
        values.insert(session.id.clone(), value);
        sessions.push(session);
    }
    Ok((sessions, values))
}

pub fn cmd_power(
    workspace: Option<&Path>,
    metric: &str,
    baseline_n: usize,
    refresh: bool,
) -> Result<()> {
    print!(
        "{}",
        exp_power_text(workspace, metric, baseline_n, refresh)?
    );
    Ok(())
}

fn window_for(e: &Experiment) -> (u64, u64) {
    let end = e
        .concluded_at_ms
        .unwrap_or_else(|| e.created_at_ms + (e.duration_days as u64) * 86_400_000);
    (e.created_at_ms, end.max(e.created_at_ms))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Deterministic UUIDv5 from experiment name + creation timestamp.
/// Stable across devices so concurrent creation of same experiment yields same ID.
fn deterministic_exp_id(name: &str, created_at_ms: u64) -> String {
    // Application-level namespace: "kaizen:experiments" hashed via UUIDv5 with DNS ns.
    const NS: uuid::Uuid = uuid::Uuid::from_bytes([
        0x6b, 0x61, 0x69, 0x7a, 0x65, 0x6e, 0x3a, 0x65, 0x78, 0x70, 0x73, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ]);
    let key = format!("{name}:{created_at_ms}");
    uuid::Uuid::new_v5(&NS, key.as_bytes()).to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
