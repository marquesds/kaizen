// SPDX-License-Identifier: AGPL-3.0-or-later
//! Config loading: workspace `.kaizen/config.toml` then `~/.kaizen/config.toml`.
//! Missing files → defaults. User config wins on overlap.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    pub roots: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            roots: vec!["~/.cursor/projects".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorSourceConfig {
    pub enabled: bool,
    pub transcript_glob: String,
}

impl Default for CursorSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            transcript_glob: "*/agent-transcripts".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourcesConfig {
    #[serde(default)]
    pub cursor: CursorSourceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    pub hot_days: u32,
    pub warm_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            hot_days: 30,
            warm_days: 90,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// When empty, sync is disabled (no outbox enqueue, `sync run` no-ops flush).
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub team_token: String,
    #[serde(default)]
    pub team_id: String,
    #[serde(default = "default_events_per_batch")]
    pub events_per_batch_max: usize,
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: usize,
    #[serde(default = "default_flush_interval_ms")]
    pub flush_interval_ms: u64,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
    /// 64 hex chars (32 bytes). Prefer `~/.kaizen/config.toml` only; never committed workspace secrets.
    #[serde(default)]
    pub team_salt_hex: String,
}

fn default_events_per_batch() -> usize {
    500
}

fn default_max_body_bytes() -> usize {
    1_000_000
}

fn default_flush_interval_ms() -> u64 {
    10_000
}

fn default_sample_rate() -> f64 {
    1.0
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            team_token: String::new(),
            team_id: String::new(),
            events_per_batch_max: default_events_per_batch(),
            max_body_bytes: default_max_body_bytes(),
            flush_interval_ms: default_flush_interval_ms(),
            sample_rate: default_sample_rate(),
            team_salt_hex: String::new(),
        }
    }
}

/// Parse `team_salt_hex` into 32 bytes. Returns `None` if missing or invalid.
pub fn try_team_salt(cfg: &SyncConfig) -> Option<[u8; 32]> {
    let h = cfg.team_salt_hex.trim();
    if h.len() != 64 {
        return None;
    }
    let bytes = hex::decode(h).ok()?;
    bytes.try_into().ok()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub sources: SourcesConfig,
    #[serde(default)]
    pub retention: RetentionConfig,
    #[serde(default)]
    pub sync: SyncConfig,
}

/// Load config: workspace `.kaizen/config.toml` then `~/.kaizen/config.toml`.
/// User config wins on overlap. Missing files → defaults, not error.
pub fn load(workspace: &Path) -> Result<Config> {
    let workspace_path = workspace.join(".kaizen/config.toml");
    let user_path = home_dir()?.join(".kaizen/config.toml");

    let base = load_file(&workspace_path).unwrap_or_default();
    let user = load_file(&user_path).unwrap_or_default();
    Ok(merge(base, user))
}

fn home_dir() -> Result<std::path::PathBuf> {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .map_err(|e| anyhow::anyhow!("HOME not set: {e}"))
}

fn load_file(path: &Path) -> Option<Config> {
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

fn merge(base: Config, user: Config) -> Config {
    Config {
        scan: if user.scan.roots != ScanConfig::default().roots {
            user.scan
        } else {
            base.scan
        },
        sources: user.sources,
        retention: user.retention,
        sync: merge_sync(base.sync, user.sync),
    }
}

fn merge_sync(base: SyncConfig, user: SyncConfig) -> SyncConfig {
    let def = SyncConfig::default();
    SyncConfig {
        endpoint: if !user.endpoint.is_empty() {
            user.endpoint
        } else {
            base.endpoint
        },
        team_token: if !user.team_token.is_empty() {
            user.team_token
        } else {
            base.team_token
        },
        team_id: if !user.team_id.is_empty() {
            user.team_id
        } else {
            base.team_id
        },
        events_per_batch_max: if user.events_per_batch_max != def.events_per_batch_max {
            user.events_per_batch_max
        } else {
            base.events_per_batch_max
        },
        max_body_bytes: if user.max_body_bytes != def.max_body_bytes {
            user.max_body_bytes
        } else {
            base.max_body_bytes
        },
        flush_interval_ms: if user.flush_interval_ms != def.flush_interval_ms {
            user.flush_interval_ms
        } else {
            base.flush_interval_ms
        },
        sample_rate: if (user.sample_rate - def.sample_rate).abs() > f64::EPSILON {
            user.sample_rate
        } else {
            base.sample_rate
        },
        team_salt_hex: if !user.team_salt_hex.is_empty() {
            user.team_salt_hex
        } else {
            base.team_salt_hex
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn defaults_when_no_files() {
        let dir = TempDir::new().unwrap();
        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.scan.roots, ScanConfig::default().roots);
        assert_eq!(cfg.retention.hot_days, 30);
    }

    #[test]
    fn workspace_config_loaded() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".kaizen")).unwrap();
        let mut f = std::fs::File::create(dir.path().join(".kaizen/config.toml")).unwrap();
        writeln!(f, "[scan]\nroots = [\"/custom/root\"]").unwrap();

        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.scan.roots, vec!["/custom/root"]);
    }

    #[test]
    fn invalid_toml_ignored() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".kaizen")).unwrap();
        std::fs::write(dir.path().join(".kaizen/config.toml"), "not valid toml :::").unwrap();

        let cfg = load(dir.path()).unwrap();
        assert_eq!(cfg.scan.roots, ScanConfig::default().roots);
    }

    #[test]
    fn merge_user_roots_win() {
        let base = Config {
            scan: ScanConfig {
                roots: vec!["/base".to_string()],
            },
            ..Default::default()
        };
        let user = Config {
            scan: ScanConfig {
                roots: vec!["/user".to_string()],
            },
            ..Default::default()
        };
        let merged = merge(base, user);
        assert_eq!(merged.scan.roots, vec!["/user"]);
    }
}
