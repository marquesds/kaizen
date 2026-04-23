// SPDX-License-Identifier: AGPL-3.0-or-later
//! Spike A — parse real Cursor/Claude/Codex transcripts.
//!
//! Walks `~/.cursor/projects/*/agent-transcripts/*.jsonl`.
//! Reports per-file and aggregate parse rates.
//! Done-signal: ≥80% parse rate; run output committed to spike-a-parser.md.

use std::{fs, path::PathBuf};

struct FileStat {
    path: PathBuf,
    total: usize,
    parsed: usize,
    blank: usize,
    errors: Vec<String>,
}

fn scan_file(path: &PathBuf) -> FileStat {
    let mut stat = FileStat {
        path: path.clone(),
        total: 0,
        parsed: 0,
        blank: 0,
        errors: Vec::new(),
    };
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            stat.errors.push(format!("read_error: {e}"));
            return stat;
        }
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            stat.blank += 1;
            continue;
        }
        stat.total += 1;
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(_) => stat.parsed += 1,
            Err(e) => {
                let msg = e.to_string();
                let short = msg.chars().take(80).collect::<String>();
                if stat.errors.len() < 5 {
                    stat.errors.push(short);
                }
            }
        }
    }
    stat
}

/// Recursively collects `.jsonl` files under `dir` (depth ≤ `max_depth`).
fn collect_jsonl(dir: &PathBuf, max_depth: u8, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() && max_depth > 0 {
            collect_jsonl(&p, max_depth - 1, files);
        } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            files.push(p);
        }
    }
}

fn find_jsonl_files(projects_dir: &PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(projects) = fs::read_dir(projects_dir) else {
        return files;
    };
    for proj in projects.flatten() {
        let transcripts = proj.path().join("agent-transcripts");
        if transcripts.is_dir() {
            // Structure: agent-transcripts/<uuid>/<uuid>.jsonl
            //            agent-transcripts/<uuid>/subagents/<uuid>.jsonl
            collect_jsonl(&transcripts, 2, &mut files);
        }
    }
    files
}

fn print_stats(stats: &[FileStat]) {
    let mut total_lines = 0usize;
    let mut total_parsed = 0usize;
    let mut total_blank = 0usize;
    let mut all_errors: std::collections::HashMap<String, usize> = Default::default();

    for s in stats {
        let rate = s
            .parsed
            .saturating_mul(100)
            .checked_div(s.total)
            .unwrap_or(100);
        println!(
            "{:3}%  {:>5}/{:<5}  {}",
            rate,
            s.parsed,
            s.total,
            s.path.display()
        );
        total_lines += s.total;
        total_parsed += s.parsed;
        total_blank += s.blank;
        for e in &s.errors {
            *all_errors.entry(e.clone()).or_default() += 1;
        }
    }

    println!("\n--- aggregate ---");
    let agg_rate = total_parsed
        .saturating_mul(100)
        .checked_div(total_lines)
        .unwrap_or(100);
    println!(
        "files: {}  lines: {}  parsed: {}  blank: {}  parse_rate: {}%",
        stats.len(),
        total_lines,
        total_parsed,
        total_blank,
        agg_rate
    );
    if !all_errors.is_empty() {
        println!("\ntop errors:");
        let mut errs: Vec<_> = all_errors.into_iter().collect();
        errs.sort_by_key(|e| std::cmp::Reverse(e.1));
        for (msg, count) in errs.iter().take(5) {
            println!("  [{count:>4}x] {msg}");
        }
    }
}

fn main() {
    let home = std::env::var("HOME").expect("HOME not set");
    let projects_dir = PathBuf::from(home).join(".cursor/projects");
    if !projects_dir.exists() {
        eprintln!("no cursor projects dir at {}", projects_dir.display());
        std::process::exit(1);
    }
    let files = find_jsonl_files(&projects_dir);
    if files.is_empty() {
        eprintln!("no .jsonl files found under {}", projects_dir.display());
        std::process::exit(1);
    }
    let stats: Vec<FileStat> = files.iter().map(scan_file).collect();
    print_stats(&stats);
}
