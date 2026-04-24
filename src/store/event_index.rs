// SPDX-License-Identifier: AGPL-3.0-or-later
//! Derive `files_touched` / `skills_used` / `rules_used` rows from ingested events.

use crate::core::event::Event;
use anyhow::Result;
use regex::Regex;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::sync::LazyLock;

static SKILL_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\.cursor/skills/([A-Za-z0-9][A-Za-z0-9_\-]{0,63})"#)
        .expect("skill path regex")
});

static RULE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\.cursor/rules/([A-Za-z0-9][A-Za-z0-9_\-]{0,63}\.mdc)"#)
        .expect("rule path regex")
});

static SLUG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^[A-Za-z0-9][A-Za-z0-9_\-]{0,63}$"#).expect("slug regex"));

/// True iff `slug` matches the strict skill/rule slug shape.
/// Used to reject legacy rows that were indexed with a permissive regex.
pub fn is_valid_slug(slug: &str) -> bool {
    SLUG_RE.is_match(slug)
}

/// Collect likely file paths from tool payload JSON (read_file, write, patch, etc.).
pub fn paths_from_event_payload(payload: &Value) -> Vec<String> {
    let mut out = Vec::new();
    collect_paths(payload, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_paths(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if matches!(
                    k.as_str(),
                    "path" | "file_path" | "target_file" | "file" | "relative_workspace_path"
                ) && val.as_str().is_some_and(|s| !s.is_empty())
                {
                    out.push(val.as_str().unwrap().to_string());
                }
                collect_paths(val, out);
            }
        }
        Value::Array(arr) => {
            for x in arr {
                collect_paths(x, out);
            }
        }
        Value::String(s) => {
            if let Ok(sub) = serde_json::from_str::<Value>(s) {
                collect_paths(&sub, out);
            }
        }
        _ => {}
    }
}

/// Skill slugs referenced via `.cursor/skills/<slug>` in stringified payload.
pub fn skills_from_event_json(payload: &Value) -> Vec<String> {
    let raw = payload.to_string();
    let mut out: Vec<String> = SKILL_PATH_RE
        .captures_iter(&raw)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn normalize_rule_id(raw: &str) -> String {
    let raw = raw.trim();
    if raw.len() > 4 && raw[raw.len() - 4..].eq_ignore_ascii_case(".mdc") {
        raw[..raw.len() - 4].to_string()
    } else {
        raw.to_string()
    }
}

/// Cursor rule stems referenced via `.cursor/rules/<name>.mdc` in stringified payload.
pub fn rules_from_event_json(payload: &Value) -> Vec<String> {
    let raw = payload.to_string();
    let mut out: Vec<String> = RULE_PATH_RE
        .captures_iter(&raw)
        .filter_map(|c| c.get(1).map(|m| normalize_rule_id(m.as_str())))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Insert derived rows after a new event was appended (`INSERT` succeeded).
pub fn index_event_derived(conn: &Connection, e: &Event) -> Result<()> {
    for path in paths_from_event_payload(&e.payload) {
        conn.execute(
            "INSERT OR IGNORE INTO files_touched (session_id, path) VALUES (?1, ?2)",
            params![e.session_id, path],
        )?;
    }
    for skill in skills_from_event_json(&e.payload) {
        conn.execute(
            "INSERT OR IGNORE INTO skills_used (session_id, skill) VALUES (?1, ?2)",
            params![e.session_id, skill],
        )?;
    }
    for rule in rules_from_event_json(&e.payload) {
        conn.execute(
            "INSERT OR IGNORE INTO rules_used (session_id, rule) VALUES (?1, ?2)",
            params![e.session_id, rule],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_from_tool_use_input() {
        let v = json!({"input": {"path": "src/main.rs"}});
        let p = paths_from_event_payload(&v);
        assert_eq!(p, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn skills_from_cursor_path_string() {
        let v = json!({"text": "read /.cursor/skills/foo/SKILL.md"});
        let s = skills_from_event_json(&v);
        assert_eq!(s, vec!["foo".to_string()]);
    }

    #[test]
    fn rules_from_cursor_rules_path() {
        let v = json!({"path": ".cursor/rules/agent-parity.mdc"});
        let r = rules_from_event_json(&v);
        assert_eq!(r, vec!["agent-parity".to_string()]);
    }

    #[test]
    fn rules_case_insensitive_mdc_suffix() {
        let v = json!({"text": "See .cursor/rules/Foo.MDC for details"});
        let r = rules_from_event_json(&v);
        assert_eq!(r, vec!["Foo".to_string()]);
    }

    #[test]
    fn skills_regex_rejects_noise_tokens() {
        // Prose mentions of `.cursor/skills/` followed by punctuation must not
        // produce garbage slugs like `**`, `{}`, `` ` ``, `\n`, `:\n1.`.
        let v = json!({
            "text": "See .cursor/skills/**, .cursor/skills/{}, .cursor/skills/`, .cursor/skills/.foo and .cursor/skills/valid-slug for details"
        });
        let s = skills_from_event_json(&v);
        assert_eq!(s, vec!["valid-slug".to_string()]);
    }

    #[test]
    fn is_valid_slug_filters_garbage() {
        assert!(is_valid_slug("kaizen-retro"));
        assert!(is_valid_slug("api_and_interface_design"));
        assert!(is_valid_slug("Foo"));
        assert!(!is_valid_slug(""));
        assert!(!is_valid_slug("`"));
        assert!(!is_valid_slug("{}"));
        assert!(!is_valid_slug("\\n"));
        assert!(!is_valid_slug(".foo"));
        assert!(!is_valid_slug("_leading"));
        assert!(!is_valid_slug("-leading"));
    }
}
