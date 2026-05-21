// SPDX-License-Identifier: AGPL-3.0-or-later
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, PartialEq)]
pub struct QueryExpr {
    pub terms: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    pub field: Field,
    pub op: Op,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Agent,
    Model,
    Kind,
    Tool,
    Path,
    Skill,
    TokensTotal,
    CostUsd,
    EvalScore,
    FeedbackLabel,
    Prompt,
    Status,
    SpanKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
}

pub fn is_structured(raw: &str) -> bool {
    raw.split_whitespace().filter(|p| *p != "AND").any(|p| {
        p.split_once(':')
            .is_some_and(|(f, _)| Field::parse(f).is_some())
    })
}

pub fn parse(raw: &str) -> Result<QueryExpr> {
    let terms = raw
        .split_whitespace()
        .filter(|p| *p != "AND")
        .map(parse_term)
        .collect::<Result<Vec<_>>>()?;
    (!terms.is_empty())
        .then_some(QueryExpr { terms })
        .ok_or_else(|| anyhow!("query needs at least one field term"))
}

fn parse_term(raw: &str) -> Result<Term> {
    let (field, rest) = raw
        .split_once(':')
        .ok_or_else(|| anyhow!("expected field:value"))?;
    let field = Field::parse(field).ok_or_else(|| anyhow!("unknown query field: {field}"))?;
    let (op, value) = Op::parse(rest);
    Ok(Term {
        field,
        op,
        value: value.to_string(),
    })
}

impl Field {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "agent" => Self::Agent,
            "model" => Self::Model,
            "kind" => Self::Kind,
            "tool" => Self::Tool,
            "path" => Self::Path,
            "skill" => Self::Skill,
            "tokens_total" => Self::TokensTotal,
            "cost_usd" => Self::CostUsd,
            "eval_score" => Self::EvalScore,
            "feedback_label" => Self::FeedbackLabel,
            "prompt" => Self::Prompt,
            "status" => Self::Status,
            "span_kind" => Self::SpanKind,
            _ => return None,
        })
    }
}

impl Op {
    fn parse(s: &str) -> (Self, &str) {
        [
            (">=", Self::Gte),
            ("<=", Self::Lte),
            (">", Self::Gt),
            ("<", Self::Lt),
            ("=", Self::Eq),
        ]
        .into_iter()
        .find_map(|(p, op)| s.strip_prefix(p).map(|v| (op, v)))
        .unwrap_or((Self::Eq, s))
    }
}
