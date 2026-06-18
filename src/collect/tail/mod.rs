// SPDX-License-Identifier: AGPL-3.0-or-later
pub mod antigravity;
pub mod claude;
pub mod claude_code;
pub mod codex;
pub mod codex_desktop;
mod codex_desktop_event;
pub mod copilot_cli;
pub mod copilot_vscode;
pub mod cursor;
pub mod cursor_state_db;
mod cursor_state_db_fields;
pub mod gemini;
pub mod goose;
pub mod kimi;
pub(crate) mod modern_jsonl;
pub(crate) mod modern_jsonl_event;
pub(crate) mod modern_jsonl_fields;
pub(crate) mod modern_jsonl_record;
pub mod openclaw;
pub mod opencode;
pub mod pi;
pub mod vibe;

pub(crate) const MAX_RECENT_TRANSCRIPTS: usize = 32;

#[cfg(test)]
mod budget_tests;

use std::cmp::Reverse;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub(crate) const MAX_TRANSCRIPT_READ_BYTES: u64 = 256 * 1024;

pub(crate) fn newest_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    newest_by_path(paths, PathBuf::as_path)
}

pub(crate) fn newest_paths_since(paths: Vec<PathBuf>, since_ms: u64) -> Vec<PathBuf> {
    newest_by_path_since(paths, PathBuf::as_path, since_ms)
}

pub(crate) fn newest_by_path<T>(mut values: Vec<T>, path: impl Fn(&T) -> &Path) -> Vec<T> {
    values.sort_by_key(|value| Reverse(modified_ms(path(value))));
    values.truncate(MAX_RECENT_TRANSCRIPTS);
    values
}

pub(crate) fn newest_by_path_since<T, F>(values: Vec<T>, path: F, since_ms: u64) -> Vec<T>
where
    F: Fn(&T) -> &Path + Copy,
{
    let recent = values
        .into_iter()
        .filter(|value| modified_ms(path(value)) >= u128::from(since_ms))
        .collect();
    newest_by_path(recent, path)
}

fn modified_ms(path: &Path) -> u128 {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_millis())
}

pub(crate) fn file_mtime_ms(path: &Path) -> u64 {
    u64::try_from(modified_ms(path)).unwrap_or(u64::MAX)
}

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn read_first_jsonl_line(path: &Path) -> std::io::Result<String> {
    let mut line = String::new();
    BufReader::new(File::open(path)?).read_line(&mut line)?;
    Ok(line)
}

pub(crate) fn read_recent_jsonl(path: &Path) -> std::io::Result<(u64, String)> {
    let file = File::open(path)?;
    let start = file
        .metadata()?
        .len()
        .saturating_sub(MAX_TRANSCRIPT_READ_BYTES);
    let mut reader = BufReader::new(file);
    let mut first_seq = count_newlines(&mut reader, start)?;
    reader.seek(SeekFrom::Start(start))?;
    first_seq += discard_partial_line(&mut reader, start)?;
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    Ok((first_seq, content))
}

fn count_newlines(reader: &mut BufReader<File>, end: u64) -> std::io::Result<u64> {
    reader.seek(SeekFrom::Start(0))?;
    let mut remaining = end;
    let mut count = 0;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = reader.read(&mut buffer[..limit])?;
        if read == 0 {
            break;
        }
        count += memchr::memchr_iter(b'\n', &buffer[..read]).count() as u64;
        remaining -= read as u64;
    }
    Ok(count)
}

fn discard_partial_line(reader: &mut BufReader<File>, start: u64) -> std::io::Result<u64> {
    if start == 0 {
        return Ok(0);
    }
    let mut ignored = Vec::new();
    reader.read_until(b'\n', &mut ignored)?;
    Ok(1)
}

/// Earliest mtime (ms) of `.jsonl` files in `dir`. Returns 0 on failure.
pub fn dir_mtime_ms(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
        })
        .min()
        .unwrap_or(0)
}

pub fn epoch_ms(t: u64) -> u64 {
    if t < 1_000_000_000_000 {
        t.saturating_mul(1000)
    } else {
        t
    }
}

pub fn value_ts_ms(v: &serde_json::Value) -> Option<u64> {
    v.as_u64()
        .map(epoch_ms)
        .or_else(|| v.as_str().and_then(rfc3339_ms))
}

fn rfc3339_ms(s: &str) -> Option<u64> {
    let dt = time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()?;
    u64::try_from(dt.unix_timestamp_nanos() / 1_000_000).ok()
}
