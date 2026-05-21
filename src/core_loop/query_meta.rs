// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::store::Store;
use anyhow::Result;

pub struct Meta {
    evals: std::collections::HashMap<String, f64>,
    feedback: std::collections::HashMap<String, String>,
    span_kinds: std::collections::HashMap<String, Vec<String>>,
}

impl Meta {
    pub fn load(store: &Store, start_ms: u64) -> Result<Self> {
        Ok(Self {
            evals: evals(store, start_ms)?,
            feedback: feedback(store, start_ms)?,
            span_kinds: span_kinds(store)?,
        })
    }

    pub fn eval(&self, id: &str) -> Option<f64> {
        self.evals.get(id).copied()
    }

    pub fn feedback(&self, id: &str) -> Option<&str> {
        self.feedback.get(id).map(String::as_str)
    }

    pub fn span_kind(&self, id: &str, kind: &str) -> bool {
        self.span_kinds
            .get(id)
            .is_some_and(|v| v.iter().any(|k| k.eq_ignore_ascii_case(kind)))
    }
}

fn evals(store: &Store, start_ms: u64) -> Result<std::collections::HashMap<String, f64>> {
    Ok(store
        .list_evals_in_window(start_ms, u64::MAX)?
        .into_iter()
        .map(|r| (r.session_id, r.score))
        .collect())
}

fn feedback(store: &Store, start_ms: u64) -> Result<std::collections::HashMap<String, String>> {
    Ok(store
        .list_feedback_in_window(start_ms, u64::MAX)?
        .into_iter()
        .filter_map(|r| r.label.map(|l| (r.session_id, l.to_string())))
        .collect())
}

fn span_kinds(store: &Store) -> Result<std::collections::HashMap<String, Vec<String>>> {
    let mut stmt = store
        .conn()
        .prepare("SELECT session_id, kind FROM trace_spans")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows
        .filter_map(|r| r.ok())
        .fold(std::collections::HashMap::new(), |mut m, (s, k)| {
            m.entry(s).or_insert_with(Vec::new).push(k);
            m
        }))
}
