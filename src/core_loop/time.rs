// SPDX-License-Identifier: AGPL-3.0-or-later
use anyhow::{Result, anyhow};

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn since_days(days: u64) -> u64 {
    now_ms().saturating_sub(days.saturating_mul(86_400_000))
}

pub fn parse_window(raw: Option<&str>, default_days: u64) -> Result<u64> {
    raw.map(parse_since)
        .transpose()?
        .map(|d| now_ms().saturating_sub(d))
        .or_else(|| Some(since_days(default_days)))
        .ok_or_else(|| anyhow!("invalid window"))
}

fn parse_since(raw: &str) -> Result<u64> {
    let s = raw.trim();
    let (n, unit) = s.split_at(s.len().saturating_sub(1));
    let n = n.parse::<u64>()?;
    match unit {
        "d" => Ok(n.saturating_mul(86_400_000)),
        "w" => Ok(n.saturating_mul(7 * 86_400_000)),
        _ => Err(anyhow!("since must end in d or w")),
    }
}
