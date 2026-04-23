// SPDX-License-Identifier: AGPL-3.0-or-later
//! Model cost estimation from bundled price table.
//! All values in cost_usd_e6 (1 USD = 1_000_000 units).

use anyhow::Result;
use serde::Deserialize;

const COST_TOML: &str = include_str!("../../assets/cost.toml");

#[derive(Debug, Deserialize)]
pub struct ModelPrice {
    pub id: String,
    pub input_per_mtok: i64,
    pub output_per_mtok: i64,
    #[serde(default)]
    pub avg_tokens_per_turn: u32,
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

    pub fn find(&self, model: &str) -> Option<&ModelPrice> {
        self.models.iter().find(|p| p.id == model)
    }
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
}
