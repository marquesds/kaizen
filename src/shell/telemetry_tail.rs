// SPDX-License-Identifier: AGPL-3.0-or-later
//! `kaizen telemetry tail` — read local NDJSON written by the `file` exporter.

use crate::shell::cli::workspace_path;
use crate::telemetry::default_ndjson_path;
use anyhow::Context;
use anyhow::Result;
use notify::RecursiveMode;
use notify::Watcher;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

/// `--file` is relative to project data when not absolute.
pub fn cmd_telemetry_tail(
    workspace: Option<&Path>,
    file: Option<PathBuf>,
    no_follow: bool,
    pretty_json: bool,
) -> Result<()> {
    let ws = workspace_path(workspace)?;
    let default_tail = file.is_none();
    let path = resolve_tail_path(&ws, file)?;
    if no_follow {
        return dump_file(&path, pretty_json, default_tail);
    }
    follow_file(&path, pretty_json)
}

fn resolve_tail_path(ws: &Path, file: Option<PathBuf>) -> Result<PathBuf> {
    match file {
        None => default_ndjson_path(ws),
        Some(p) if p.is_absolute() => Ok(p),
        Some(p) => Ok(crate::core::paths::project_data_path(ws)?.join(p)),
    }
}

fn print_line(line: &str, pretty: bool) -> Result<()> {
    if pretty {
        let v: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("line is not valid JSON: {line:?}"))?;
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        println!("{line}");
    }
    std::io::stdout().flush()?;
    Ok(())
}

fn dump_file(path: &Path, pretty: bool, missing_is_empty: bool) -> Result<()> {
    if missing_is_empty && !path.exists() {
        return Ok(());
    }
    let f = std::fs::File::open(path).with_context(|| {
        format!(
            "open {} (set --file or use workspace default; create lines via [[telemetry.exporters]] type = \"file\")",
            path.display()
        )
    })?;
    for line in std::io::BufReader::new(f).lines() {
        print_line(&line?, pretty)?;
    }
    Ok(())
}

fn follow_file(path: &Path, pretty: bool) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut w = notify::recommended_watcher(move |e| {
        let _ = tx.send(e);
    })
    .with_context(|| format!("watcher for {}", parent.display()))?;
    w.watch(&parent, RecursiveMode::NonRecursive)
        .with_context(|| format!("watch {}", parent.display()))?;
    let _keep = w;
    let mut off = 0u64;
    loop {
        read_appended(path, &mut off, pretty)?;
        let _ = rx.recv_timeout(Duration::from_millis(400));
    }
}

fn read_appended(path: &Path, off: &mut u64, pretty: bool) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let len = std::fs::metadata(path)?.len();
    if *off > len {
        *off = 0;
    }
    let mut f = std::fs::File::open(path)?;
    std::io::Seek::seek(&mut f, std::io::SeekFrom::Start(*off))?;
    let mut r = std::io::BufReader::new(f);
    let mut line = String::new();
    while r.read_line(&mut line)? > 0 {
        print_line(line.trim_end_matches(['\n', '\r']), pretty)?;
        line.clear();
    }
    *off = std::fs::metadata(path)?.len();
    Ok(())
}
