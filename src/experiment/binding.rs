// SPDX-License-Identifier: AGPL-3.0-or-later
//! Classify sessions into Control / Treatment / Excluded under a binding.
//!
//! Resolution order: manual tag > git ancestry. Branch-binding is a
//! special case of GitCommit: caller resolves branch tips to commits.

use crate::core::event::SessionRecord;
use crate::experiment::types::{Binding, Classification};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Per-experiment manual tags keyed by `session_id` → variant.
pub type ManualTags = HashMap<String, Classification>;

/// Classify a session. Manual tag wins over git binding.
///
/// Pure w.r.t. `session` + `manual_tags`; shells out to `git` only when a
/// `GitCommit` binding is in effect and no manual tag overrides.
pub fn classify(
    session: &SessionRecord,
    binding: &Binding,
    manual_tags: &ManualTags,
    workspace: &Path,
) -> Classification {
    if let Some(v) = manual_tags.get(&session.id) {
        return v.clone();
    }
    match binding {
        Binding::ManualTag { .. } => Classification::Excluded,
        Binding::GitCommit {
            control_commit,
            treatment_commit,
        } => classify_git(session, control_commit, treatment_commit, workspace),
        Binding::Branch {
            control_branch,
            treatment_branch,
        } => classify_git(session, control_branch, treatment_branch, workspace),
    }
}

fn classify_git(
    session: &SessionRecord,
    control_commit: &str,
    treatment_commit: &str,
    workspace: &Path,
) -> Classification {
    let Some(start) = session.start_commit.as_deref() else {
        return Classification::Excluded;
    };
    let on_treatment = is_ancestor(workspace, start, treatment_commit).unwrap_or(false);
    let on_control = is_ancestor(workspace, start, control_commit).unwrap_or(false);
    match (on_treatment, on_control) {
        // strictly descended from control (not yet at treatment boundary)
        (false, true) => Classification::Control,
        // past the treatment boundary
        (true, false) => Classification::Treatment,
        // straddles or unknown
        _ => Classification::Excluded,
    }
}

fn is_ancestor(workspace: &Path, maybe_ancestor: &str, descendant: &str) -> Result<bool> {
    if maybe_ancestor == descendant {
        return Ok(true);
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["merge-base", "--is-ancestor", maybe_ancestor, descendant])
        .output()?;
    Ok(out.status.success())
}

/// Partition sessions by classification, preserving input order.
pub fn partition<'a>(
    sessions: &'a [SessionRecord],
    binding: &Binding,
    manual_tags: &ManualTags,
    workspace: &Path,
) -> (
    Vec<&'a SessionRecord>,
    Vec<&'a SessionRecord>,
    Vec<&'a SessionRecord>,
) {
    let mut control = Vec::new();
    let mut treatment = Vec::new();
    let mut excluded = Vec::new();
    for s in sessions {
        match classify(s, binding, manual_tags, workspace) {
            Classification::Control => control.push(s),
            Classification::Treatment => treatment.push(s),
            Classification::Excluded => excluded.push(s),
        }
    }
    (control, treatment, excluded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::SessionStatus;

    fn mk(id: &str, commit: Option<&str>) -> SessionRecord {
        SessionRecord {
            id: id.into(),
            agent: "cursor".into(),
            model: None,
            workspace: "/ws".into(),
            started_at_ms: 0,
            ended_at_ms: None,
            status: SessionStatus::Done,
            trace_path: String::new(),
            start_commit: commit.map(Into::into),
            end_commit: None,
            branch: None,
            dirty_start: None,
            dirty_end: None,
            repo_binding_source: None,
            prompt_fingerprint: None,
        }
    }

    #[test]
    fn manual_tag_beats_git_binding() {
        let s = mk("s1", Some("abc"));
        let binding = Binding::GitCommit {
            control_commit: "c".into(),
            treatment_commit: "t".into(),
        };
        let mut tags = ManualTags::new();
        tags.insert("s1".into(), Classification::Treatment);
        let got = classify(&s, &binding, &tags, Path::new("/no"));
        assert_eq!(got, Classification::Treatment);
    }

    #[test]
    fn no_start_commit_excludes() {
        let s = mk("s1", None);
        let binding = Binding::GitCommit {
            control_commit: "c".into(),
            treatment_commit: "t".into(),
        };
        let tags = ManualTags::new();
        let got = classify(&s, &binding, &tags, Path::new("/no"));
        assert_eq!(got, Classification::Excluded);
    }

    #[test]
    fn partition_splits_three_ways() {
        let s1 = mk("1", None);
        let s2 = mk("2", None);
        let s3 = mk("3", None);
        let all = vec![s1, s2, s3];
        let binding = Binding::GitCommit {
            control_commit: "c".into(),
            treatment_commit: "t".into(),
        };
        let mut tags = ManualTags::new();
        tags.insert("1".into(), Classification::Control);
        tags.insert("2".into(), Classification::Treatment);
        let (c, t, e) = partition(&all, &binding, &tags, Path::new("/no"));
        assert_eq!(c.len(), 1);
        assert_eq!(t.len(), 1);
        assert_eq!(e.len(), 1);
    }
}
