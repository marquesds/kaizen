// SPDX-License-Identifier: AGPL-3.0-or-later
//! Atomic report files + Markdown rendering for retro.

use crate::retro::types::{Bet, BetCategory, Confidence, Report};
use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

/// ISO week label e.g. `2026-W17` (UTC).
pub fn iso_week_label_utc() -> String {
    let d = time::OffsetDateTime::now_utc().date();
    format!("{}-W{:02}", d.year(), d.iso_week())
}

/// Serialize report as JSON for `kaizen retro --json`.
pub fn to_json(report: &Report) -> Result<String> {
    Ok(serde_json::to_string_pretty(report)?)
}

/// Markdown matching `docs/retro.md` output shape.
pub fn to_markdown(report: &Report) -> String {
    let mut s = String::new();
    let cost = report.meta.total_cost_usd_e6 as f64 / 1_000_000.0;
    let title_week = if report.meta.week_label.is_empty() {
        "—"
    } else {
        report.meta.week_label.as_str()
    };
    s.push_str(&format!("# Kaizen Retro — Week {}\n\n", title_week));
    s.push_str(&format!(
        "Span: {} → {} · Sessions: {} · Cost: ${:.2}\n\n",
        report.meta.span_start_ms, report.meta.span_end_ms, report.meta.session_count, cost
    ));
    let mut index = 1;
    let high = report
        .top_bets
        .iter()
        .position(|b| b.confidence == Some(Confidence::High));
    if let Some(high_idx) = high {
        render_section(
            &mut s,
            "High-Confidence Bet",
            report.top_bets[high_idx..=high_idx].iter(),
            &mut index,
        );
    } else {
        s.push_str(
            "> No high-confidence bet this window; treat remaining bets as exploratory.\n\n",
        );
    }
    render_section(
        &mut s,
        "To Investigate",
        report.top_bets.iter().enumerate().filter_map(|(i, b)| {
            (Some(i) != high && b.category == Some(BetCategory::Investigation)).then_some(b)
        }),
        &mut index,
    );
    render_section(
        &mut s,
        "Quick Hygiene",
        report.top_bets.iter().enumerate().filter_map(|(i, b)| {
            (Some(i) != high
                && matches!(
                    b.category,
                    None | Some(BetCategory::QuickWin | BetCategory::Hygiene)
                ))
            .then_some(b)
        }),
        &mut index,
    );
    if !report.skipped_deduped.is_empty() {
        s.push_str("## Skipped Bets (deduped vs prior reports)\n\n");
        for line in &report.skipped_deduped {
            s.push_str(&format!("- {}\n", line));
        }
        s.push('\n');
    }
    s.push_str("## Raw Stats\n\n");
    s.push_str("| Metric | Value |\n|---|---|\n");
    s.push_str(&format!("| Sessions | {} |\n", report.stats.sessions));
    s.push_str(&format!(
        "| Total cost | ${:.2} |\n",
        report.stats.total_cost_usd_e6 as f64 / 1_000_000.0
    ));
    if let Some(ref m) = report.stats.top_model {
        let p = report
            .stats
            .top_model_pct
            .map(|x| format!("{}%", x))
            .unwrap_or_else(|| "—".into());
        s.push_str(&format!("| Top model | {} ({}) |\n", m, p));
    }
    if let Some(ref t) = report.stats.top_tool {
        let p = report
            .stats
            .top_tool_pct
            .map(|x| format!("{}%", x))
            .unwrap_or_else(|| "—".into());
        s.push_str(&format!("| Top tool | {} ({}) |\n", t, p));
    }
    if let Some(med) = report.stats.median_session_minutes {
        s.push_str(&format!("| Median session | {} min |\n", med));
    }
    s
}

fn render_section<'a, I>(s: &mut String, title: &str, bets: I, index: &mut usize)
where
    I: Iterator<Item = &'a Bet>,
{
    let start = *index;
    for bet in bets {
        if *index == start {
            s.push_str(&format!("## {}\n\n", title));
        }
        render_bet(s, bet, index);
    }
}

fn render_bet(s: &mut String, bet: &Bet, index: &mut usize) {
    s.push_str(&format!(
        "### {}. {} ({} · {} · {})\n",
        *index,
        bet.title,
        bet.heuristic_id,
        confidence_label(bet),
        category_label(bet)
    ));
    s.push_str(&format!("- Hypothesis: {}\n", bet.hypothesis));
    render_evidence(s, bet);
    s.push_str(&format!(
        "- Saves ~{:.0} tokens/week (est.) · Confidence: {}\n",
        bet.expected_tokens_saved_per_week,
        confidence_label(bet)
    ));
    s.push_str(&format!(
        "- Effort: {} min · Apply: {}\n\n",
        bet.effort_minutes, bet.apply_step
    ));
    *index += 1;
}

fn render_evidence(s: &mut String, bet: &Bet) {
    if bet.evidence.is_empty() {
        return;
    }
    s.push_str(&format!("- Evidence: {}\n", bet.evidence.join(" · ")));
}

fn confidence_label(bet: &Bet) -> &'static str {
    bet.confidence.map_or("Unknown", Confidence::label)
}

fn category_label(bet: &Bet) -> &'static str {
    bet.category.map_or("unknown", BetCategory::label)
}

/// Write bytes to `path` via temp file + rename.
pub fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let mut f = File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
    f.write_all(content)?;
    f.sync_all().ok();
    drop(f);
    fs::rename(&tmp, path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Exclusive lock for the reports directory (released when the inner [`File`] is closed on drop).
pub struct ReportsDirLock(#[allow(dead_code)] File);

impl ReportsDirLock {
    pub fn acquire(reports_dir: &Path) -> Result<Self> {
        fs::create_dir_all(reports_dir)?;
        let p = reports_dir.join(".retro.lock");
        let f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&p)
            .with_context(|| format!("lock {}", p.display()))?;
        fs4::FileExt::lock(&f).with_context(|| format!("lock {}", p.display()))?;
        Ok(Self(f))
    }
}
