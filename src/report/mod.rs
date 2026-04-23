//! Atomic report files + Markdown rendering for retro.

use crate::retro::types::Report;
use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
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
    s.push_str("## Top Bets\n\n");
    for (i, b) in report.top_bets.iter().enumerate() {
        s.push_str(&format!("### {}. {} ({})\n", i + 1, b.title, b.id));
        s.push_str(&format!("- Hypothesis: {}\n", b.hypothesis));
        s.push_str(&format!(
            "- Saves ~{:.0} tokens/week (est.)\n",
            b.expected_tokens_saved_per_week
        ));
        s.push_str(&format!("- Effort: {} min\n", b.effort_minutes));
        if !b.evidence.is_empty() {
            s.push_str("- Evidence:\n");
            for e in &b.evidence {
                s.push_str(&format!("  - {}\n", e));
            }
        }
        s.push_str(&format!("- Apply: {}\n\n", b.apply_step));
    }
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

/// Exclusive lock for the reports directory (released on drop).
pub struct ReportsDirLock(File);

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
        f.lock_exclusive()
            .with_context(|| format!("lock_exclusive {}", p.display()))?;
        Ok(Self(f))
    }
}

impl Drop for ReportsDirLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}
