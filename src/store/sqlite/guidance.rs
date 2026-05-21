use super::*;

impl Store {
    /// Skill/rule adoption and cost proxy vs workspace average (observed payload references only).
    pub fn guidance_report(
        &self,
        workspace: &str,
        window_start_ms: u64,
        window_end_ms: u64,
        skill_slugs_on_disk: &HashSet<String>,
        rule_slugs_on_disk: &HashSet<String>,
    ) -> Result<GuidanceReport> {
        let active = self.sessions_active_in_window(workspace, window_start_ms, window_end_ms)?;
        let denom = active.len() as u64;
        let costs =
            self.session_costs_usd_e6_in_window(workspace, window_start_ms, window_end_ms)?;

        let workspace_avg_cost_per_session_usd = if denom > 0 {
            let total_e6: i64 = active
                .iter()
                .map(|sid| costs.get(sid).copied().unwrap_or(0))
                .sum();
            Some(total_e6 as f64 / denom as f64 / 1_000_000.0)
        } else {
            None
        };

        let mut skill_sessions: HashMap<String, HashSet<String>> = HashMap::new();
        for (sid, skill) in self.skills_used_in_window(workspace, window_start_ms, window_end_ms)? {
            skill_sessions.entry(skill).or_default().insert(sid);
        }
        let mut rule_sessions: HashMap<String, HashSet<String>> = HashMap::new();
        for (sid, rule) in self.rules_used_in_window(workspace, window_start_ms, window_end_ms)? {
            rule_sessions.entry(rule).or_default().insert(sid);
        }

        let mut rows: Vec<GuidancePerfRow> = Vec::new();

        let mut push_row =
            |kind: GuidanceKind, id: String, sids: &HashSet<String>, on_disk: bool| {
                let sessions = sids.len() as u64;
                let sessions_pct = if denom > 0 {
                    sessions as f64 * 100.0 / denom as f64
                } else {
                    0.0
                };
                let total_cost_usd_e6: i64 = sids
                    .iter()
                    .map(|sid| costs.get(sid).copied().unwrap_or(0))
                    .sum();
                let avg_cost_per_session_usd = if sessions > 0 {
                    Some(total_cost_usd_e6 as f64 / sessions as f64 / 1_000_000.0)
                } else {
                    None
                };
                let vs_workspace_avg_cost_per_session_usd =
                    match (avg_cost_per_session_usd, workspace_avg_cost_per_session_usd) {
                        (Some(avg), Some(w)) => Some(avg - w),
                        _ => None,
                    };
                rows.push(GuidancePerfRow {
                    kind,
                    id,
                    sessions,
                    sessions_pct,
                    total_cost_usd_e6,
                    avg_cost_per_session_usd,
                    vs_workspace_avg_cost_per_session_usd,
                    on_disk,
                });
            };

        let mut seen_skills: HashSet<String> = HashSet::new();
        for (id, sids) in &skill_sessions {
            seen_skills.insert(id.clone());
            push_row(
                GuidanceKind::Skill,
                id.clone(),
                sids,
                skill_slugs_on_disk.contains(id),
            );
        }
        for slug in skill_slugs_on_disk {
            if seen_skills.contains(slug) {
                continue;
            }
            push_row(GuidanceKind::Skill, slug.clone(), &HashSet::new(), true);
        }

        let mut seen_rules: HashSet<String> = HashSet::new();
        for (id, sids) in &rule_sessions {
            seen_rules.insert(id.clone());
            push_row(
                GuidanceKind::Rule,
                id.clone(),
                sids,
                rule_slugs_on_disk.contains(id),
            );
        }
        for slug in rule_slugs_on_disk {
            if seen_rules.contains(slug) {
                continue;
            }
            push_row(GuidanceKind::Rule, slug.clone(), &HashSet::new(), true);
        }

        rows.sort_by(|a, b| {
            b.sessions
                .cmp(&a.sessions)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(GuidanceReport {
            workspace: workspace.to_string(),
            window_start_ms,
            window_end_ms,
            sessions_in_window: denom,
            workspace_avg_cost_per_session_usd,
            rows,
        })
    }
}
