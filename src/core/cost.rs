// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model cost estimation from bundled price table.
//! All values in cost_usd_e6 (1 USD = 1_000_000 units).

use anyhow::Result;
use serde::Deserialize;
use std::sync::OnceLock;

const COST_TOML: &str = include_str!("../../assets/cost.toml");

#[derive(Debug, Deserialize)]
pub struct ModelPrice {
    pub id: String,
    pub input_per_mtok: i64,
    pub output_per_mtok: i64,
    #[serde(default)]
    pub cache_read_per_mtok: Option<i64>,
    #[serde(default)]
    pub cache_create_per_mtok: Option<i64>,
    #[serde(default)]
    pub avg_tokens_per_turn: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TokenUsage {
    pub input: u32,
    pub output: u32,
    pub reasoning: u32,
    pub cache_read: u32,
    pub cache_create: u32,
}

#[derive(Debug, Deserialize)]
struct CostFile {
    models: Vec<ModelPrice>,
}

pub struct CostTable {
    models: Vec<ModelPrice>,
}

impl CostTable {
    /// Load from bundled `assets/cost.toml`.
    pub fn load() -> Result<Self> {
        let f: CostFile = toml::from_str(COST_TOML)?;
        Ok(Self { models: f.models })
    }

    /// Estimate cost in cost_usd_e6 units.
    /// If model is None or not found, falls back to "cursor" heuristic entry.
    /// If tokens_in == 0 and tokens_out == 0 (Cursor: no native tokens),
    /// uses avg_tokens_per_turn from the matched entry.
    pub fn estimate(&self, model: Option<&str>, tokens_in: u32, tokens_out: u32) -> i64 {
        let entry = model
            .and_then(|m| self.models.iter().find(|p| p.id == m))
            .or_else(|| self.models.iter().find(|p| p.id == "cursor"));

        let Some(price) = entry else { return 0 };

        let (tin, tout) = if tokens_in == 0 && tokens_out == 0 && price.avg_tokens_per_turn > 0 {
            let avg = price.avg_tokens_per_turn as i64;
            (avg * 4 / 5, avg / 5)
        } else {
            (tokens_in as i64, tokens_out as i64)
        };

        tin * price.input_per_mtok / 1_000_000 + tout * price.output_per_mtok / 1_000_000
    }

    pub fn estimate_usage_cost_usd_e6(&self, model: Option<&str>, usage: TokenUsage) -> i64 {
        let Some(price) = self.price_for(model) else {
            return 0;
        };
        usage_cost(price, usage)
    }

    pub fn find(&self, model: &str) -> Option<&ModelPrice> {
        self.models.iter().find(|p| p.id == model)
    }

    /// Transcript tail rows: charge only when at least one usage field is set **and**
    /// prompt + output (including reasoning) are not all zero. Omits proxy-style
    /// `avg_tokens_per_turn` fallback so thousands of tool lines do not each get a heuristic charge.
    pub fn estimate_tail_event_cost_usd_e6(
        &self,
        model: Option<&str>,
        tokens_in: Option<u32>,
        tokens_out: Option<u32>,
        reasoning_tokens: Option<u32>,
    ) -> Option<i64> {
        let any_field = tokens_in.is_some() || tokens_out.is_some() || reasoning_tokens.is_some();
        if !any_field {
            return None;
        }
        let tin = tokens_in.unwrap_or(0);
        let tout = tokens_out
            .unwrap_or(0)
            .saturating_add(reasoning_tokens.unwrap_or(0));
        if tin == 0 && tout == 0 {
            return None;
        }
        Some(self.estimate(model, tin, tout))
    }

    pub fn estimate_tail_usage_cost_usd_e6(
        &self,
        model: Option<&str>,
        usage: TokenUsage,
    ) -> Option<i64> {
        usage
            .any()
            .then(|| self.estimate_usage_cost_usd_e6(model, usage))
    }

    fn price_for(&self, model: Option<&str>) -> Option<&ModelPrice> {
        model
            .and_then(|m| self.models.iter().find(|p| p.id == m))
            .or_else(|| self.models.iter().find(|p| p.id == "cursor"))
    }
}

impl TokenUsage {
    pub fn from_tail(input: Option<u32>, output: Option<u32>, reasoning: Option<u32>) -> Self {
        Self {
            input: input.unwrap_or(0),
            output: output.unwrap_or(0),
            reasoning: reasoning.unwrap_or(0),
            cache_read: 0,
            cache_create: 0,
        }
    }

    fn any(self) -> bool {
        self.input > 0
            || self.output > 0
            || self.reasoning > 0
            || self.cache_read > 0
            || self.cache_create > 0
    }
}

fn usage_cost(price: &ModelPrice, usage: TokenUsage) -> i64 {
    let out = usage.output.saturating_add(usage.reasoning) as i64;
    let cache_read = price.cache_read_per_mtok.unwrap_or(price.input_per_mtok);
    let cache_create = price.cache_create_per_mtok.unwrap_or(price.input_per_mtok);
    usage.input as i64 * price.input_per_mtok / 1_000_000
        + out * price.output_per_mtok / 1_000_000
        + usage.cache_read as i64 * cache_read / 1_000_000
        + usage.cache_create as i64 * cache_create / 1_000_000
}

static BUNDLED_COST: OnceLock<CostTable> = OnceLock::new();

fn bundled_cost_table() -> &'static CostTable {
    BUNDLED_COST.get_or_init(|| CostTable::load().expect("bundled assets/cost.toml"))
}

/// [`CostTable::estimate_tail_event_cost_usd_e6`] on the bundled table.
pub fn estimate_tail_event_cost_usd_e6(
    model: Option<&str>,
    tokens_in: Option<u32>,
    tokens_out: Option<u32>,
    reasoning_tokens: Option<u32>,
) -> Option<i64> {
    bundled_cost_table().estimate_tail_event_cost_usd_e6(
        model,
        tokens_in,
        tokens_out,
        reasoning_tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_succeeds() {
        CostTable::load().unwrap();
    }

    #[test]
    fn known_model_cost() {
        let table = CostTable::load().unwrap();
        // claude-sonnet-4: $3/MTok in, $15/MTok out
        // 1000 in + 500 out → 3000 + 7500 = 10500 cost_usd_e6
        let cost = table.estimate(Some("claude-sonnet-4"), 1000, 500);
        assert_eq!(cost, 10500);
    }

    #[test]
    fn cursor_heuristic_nonzero() {
        let table = CostTable::load().unwrap();
        // model=None, no tokens → cursor heuristic
        let cost = table.estimate(None, 0, 0);
        assert!(cost > 0, "cursor heuristic should produce nonzero cost");
    }

    #[test]
    fn unknown_model_falls_back_to_cursor() {
        let table = CostTable::load().unwrap();
        let cost = table.estimate(Some("unknown-model-xyz"), 1000, 500);
        let cost2 = table.estimate(None, 1000, 500);
        assert_eq!(cost, cost2);
    }

    #[test]
    fn tail_estimate_none_without_usage_fields() {
        let table = CostTable::load().unwrap();
        assert!(
            table
                .estimate_tail_event_cost_usd_e6(None, None, None, None)
                .is_none()
        );
    }

    #[test]
    fn tail_estimate_none_when_fields_present_but_all_zero() {
        let table = CostTable::load().unwrap();
        assert!(table
            .estimate_tail_event_cost_usd_e6(
                Some("claude-sonnet-4"),
                Some(0),
                Some(0),
                Some(0),
            )
            .is_none());
    }

    #[test]
    fn tail_estimate_adds_reasoning_to_output_side() {
        let table = CostTable::load().unwrap();
        let with_reasoning = table
            .estimate_tail_event_cost_usd_e6(
                Some("claude-sonnet-4"),
                Some(1000),
                Some(100),
                Some(400),
            )
            .expect("cost");
        let output_only = table
            .estimate_tail_event_cost_usd_e6(Some("claude-sonnet-4"), Some(1000), Some(500), None)
            .expect("cost");
        assert_eq!(with_reasoning, output_only);
    }

    #[test]
    fn usage_estimate_prices_cache_tokens_separately() {
        let table = CostTable::load().unwrap();
        let cached = table.estimate_usage_cost_usd_e6(
            Some("claude-sonnet-4"),
            TokenUsage {
                input: 1000,
                output: 500,
                reasoning: 0,
                cache_read: 1000,
                cache_create: 1000,
            },
        );
        let plain = table.estimate(Some("claude-sonnet-4"), 3000, 500);
        assert!(cached < plain);
    }

    #[test]
    fn tail_estimate_includes_cache_usage() {
        let table = CostTable::load().unwrap();
        let cached = table
            .estimate_tail_usage_cost_usd_e6(
                Some("claude-sonnet-4"),
                TokenUsage {
                    input: 0,
                    output: 0,
                    reasoning: 0,
                    cache_read: 1000,
                    cache_create: 0,
                },
            )
            .expect("cache read cost");
        assert!(cached > 0);
    }
}
