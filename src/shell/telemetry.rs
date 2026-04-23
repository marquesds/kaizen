// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen telemetry configure` and `print-effective-config`.

use crate::core::config::{self, ExporterConfig};
use crate::shell::cli::workspace_path;
use crate::telemetry::{DatadogResolved, OtlpResolved, PostHogResolved};
use anyhow::{Context, Result};
use std::io::{BufRead, Write};
use std::path::Path;

/// Interactive: append a PostHog stub row to `~/.kaizen/config.toml` (keys via env or paste).
pub fn cmd_telemetry_configure(workspace: Option<&Path>) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let home = std::env::var("HOME").context("HOME not set")?;
    let p = std::path::PathBuf::from(home).join(".kaizen/config.toml");
    std::fs::create_dir_all(p.parent().unwrap())?;

    println!("Kaizen pluggable telemetry — optional sinks fan-out alongside Kaizen sync.");
    println!(
        "This command appends a `[[telemetry.exporters]]` table to {}.",
        p.display()
    );
    print!("Type `posthog`, `datadog`, `otlp`, or `dev` (or empty to abort): ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let t = line.trim().to_lowercase();
    if t.is_empty() {
        println!("Aborted.");
        return Ok(());
    }
    let block = match t.as_str() {
        "posthog" => {
            r#"
[[telemetry.exporters]]
type = "posthog"
# project_api_key = "phc_..."  # or set POSTHOG_API_KEY
# host = "https://us.i.posthog.com"
"#
        }
        "datadog" => {
            r#"
[[telemetry.exporters]]
type = "datadog"
# api_key = "..."   # or set DD_API_KEY
# site = "datadoghq.com"
"#
        }
        "otlp" => {
            r#"
[[telemetry.exporters]]
type = "otlp"
# endpoint = "http://127.0.0.1:4318"  # or set OTEL_EXPORTER_OTLP_ENDPOINT
"#
        }
        "dev" => {
            r#"
[[telemetry.exporters]]
type = "dev"
"#
        }
        _ => anyhow::bail!("unknown type (use posthog, datadog, otlp, dev)"),
    };

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)?;
    if f.metadata()?.len() > 0 {
        f.write_all(b"\n")?;
    }
    f.write_all(block.as_bytes())?;
    let _ = ws;
    println!("Appended. Rebuild with e.g. `--features telemetry-posthog` for PostHog.");
    Ok(())
}

/// Redacted: show which env/Toml fields are visible for `telemetry` sinks.
pub fn print_effective_config_text(workspace: Option<&Path>) -> Result<String> {
    let ws = workspace_path(workspace)?;
    let cfg = config::load(&ws)?;
    use std::fmt::Write;
    let mut s = String::new();
    writeln!(&mut s, "telemetry.fail_open: {}", cfg.telemetry.fail_open).unwrap();
    for (i, e) in cfg.telemetry.exporters.iter().enumerate() {
        match e {
            ExporterConfig::None => writeln!(&mut s, "[{i}] type=none (ignored)").unwrap(),
            ExporterConfig::Dev { enabled } => {
                writeln!(&mut s, "[{i}] type=dev enabled={enabled}").unwrap();
            }
            ExporterConfig::PostHog { .. } => {
                let line = if let Some(r) = PostHogResolved::from_config(e) {
                    format!(
                        "[{i}] type=posthog host={} key=<redacted len {}>",
                        r.host,
                        r.project_api_key.len()
                    )
                } else {
                    format!(
                        "[{i}] type=posthog (unresolved: set POSTHOG_API_KEY or project_api_key)"
                    )
                };
                writeln!(&mut s, "{line}").unwrap();
            }
            ExporterConfig::Datadog { .. } => {
                let line = if let Some(r) = DatadogResolved::from_config(e) {
                    format!(
                        "[{i}] type=datadog site={} key=<redacted len {}>",
                        r.site,
                        r.api_key.len()
                    )
                } else {
                    format!("[{i}] type=datadog (unresolved: set DD_API_KEY or api_key in TOML)")
                };
                writeln!(&mut s, "{line}").unwrap();
            }
            ExporterConfig::Otlp { .. } => {
                let line = if let Some(r) = OtlpResolved::from_config(e) {
                    format!("[{i}] type=otlp endpoint={}", r.endpoint)
                } else {
                    format!("[{i}] type=otlp (unresolved: OTEL_EXPORTER_OTLP_ENDPOINT)")
                };
                writeln!(&mut s, "{line}").unwrap();
            }
        }
    }
    if cfg.telemetry.exporters.is_empty() {
        writeln!(&mut s, "(no [[telemetry.exporters]] rows)").unwrap();
    }
    Ok(s)
}

pub fn cmd_telemetry_print_effective(workspace: Option<&Path>) -> Result<()> {
    print!("{}", print_effective_config_text(workspace)?);
    Ok(())
}
