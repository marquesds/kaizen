// SPDX-License-Identifier: AGPL-3.0-or-later
//! H6 — Skill trigger misfire (`outcome = ignored` in payloads).

use crate::retro::types::{Bet, Inputs};

/// Requires explicit `outcome` / `ignored` markers in payloads; otherwise silent.
pub fn run(inputs: &Inputs) -> Vec<Bet> {
    let mut ignored = 0u64;
    let mut total = 0u64;
    let mut last_skill = String::new();
    for (_, e) in &inputs.events {
        let raw = e.payload.to_string();
        if !raw.contains("skill") && !raw.contains("Skill") {
            continue;
        }
        total += 1;
        let low = raw.to_lowercase();
        if low.contains("ignored") || low.contains("\"outcome\":\"ignored\"") {
            ignored += 1;
            if let Some(m) = raw.split(".cursor/skills/").nth(1) {
                last_skill = m.split('/').next().unwrap_or("").to_string();
            }
        }
    }
    if total < 10 || ignored * 100 / total < 70 {
        return vec![];
    }
    let slug = if last_skill.is_empty() {
        "unknown"
    } else {
        last_skill.as_str()
    };
    vec![Bet {
        id: format!("H6:{slug}"),
        heuristic_id: "H6".into(),
        title: format!("Skill `{}` often ignored", slug),
        hypothesis: format!(
            "{:.0}% of skill-related hook/tool payloads look ignored — description may be misfiring.",
            (ignored as f64) * 100.0 / (total as f64)
        ),
        expected_tokens_saved_per_week: (ignored as f64) * 150.0,
        effort_minutes: 20,
        evidence: vec![format!("Ignored-like payloads: {} / {}", ignored, total)],
        apply_step: format!(
            "Rewrite `.cursor/skills/{}/SKILL.md` description frontmatter to tighter triggers.",
            slug
        ),
        evidence_recency_ms: inputs.window_end_ms,
        confidence: None,
        category: None,
    }]
}
