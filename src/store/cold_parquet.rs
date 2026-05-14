// SPDX-License-Identifier: AGPL-3.0-or-later
//! COLD Parquet partitions for event-shaped rows.

use crate::core::event::{Event, EventKind, EventSource};
use anyhow::Result;
use arrow::array::{
    Array, ArrayRef, BooleanArray, Int64Array, StringArray, UInt16Array, UInt32Array, UInt64Array,
};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::metadata::KeyValue;
use parquet::file::properties::WriterProperties;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use time::{OffsetDateTime, format_description::well_known::Iso8601};

pub const SCHEMA_VERSION: &str = "1";

pub struct DailyEventWriter {
    root: PathBuf,
    max_rows: usize,
    groups: BTreeMap<String, Vec<Event>>,
    next_chunk: BTreeMap<String, u64>,
    paths: Vec<PathBuf>,
}

impl DailyEventWriter {
    pub fn new(root: &Path, max_rows: usize) -> Self {
        Self {
            root: root.to_path_buf(),
            max_rows: max_rows.max(1),
            groups: BTreeMap::new(),
            next_chunk: BTreeMap::new(),
            paths: Vec::new(),
        }
    }

    pub fn push(&mut self, event: Event) -> Result<()> {
        let day = partition_day(event.ts_ms)?;
        let full = push_group(&mut self.groups, day.clone(), event, self.max_rows);
        if full {
            self.flush_day(&day)?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<PathBuf>> {
        while let Some(day) = self.groups.keys().next().cloned() {
            self.flush_day(&day)?;
        }
        Ok(self.paths)
    }

    fn flush_day(&mut self, day: &str) -> Result<()> {
        let Some(rows) = self.groups.remove(day) else {
            return Ok(());
        };
        let path = self.next_chunk_path(day)?;
        write_batch(&path, &rows)?;
        self.paths.push(path);
        Ok(())
    }

    fn next_chunk_path(&mut self, day: &str) -> Result<PathBuf> {
        let dir = self.root.join("cold/events");
        std::fs::create_dir_all(&dir)?;
        let n = self.next_chunk.entry(day.to_string()).or_default();
        let path = dir.join(format!("{day}-{n:06}.parquet"));
        *n += 1;
        Ok(path)
    }
}

fn push_group(
    groups: &mut BTreeMap<String, Vec<Event>>,
    day: String,
    event: Event,
    max_rows: usize,
) -> bool {
    let rows = groups.entry(day).or_default();
    rows.push(event);
    rows.len() >= max_rows
}

pub fn write_daily_events(root: &Path, events: &[Event]) -> Result<Vec<PathBuf>> {
    let mut groups: BTreeMap<String, Vec<Event>> = BTreeMap::new();
    for event in events {
        groups
            .entry(partition_day(event.ts_ms)?)
            .or_default()
            .push(event.clone());
    }
    write_daily_event_groups(root, groups)
}

pub fn write_daily_event_groups(
    root: &Path,
    groups: BTreeMap<String, Vec<Event>>,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (day, rows) in groups {
        let dir = root.join("cold/events");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{day}.parquet"));
        write_batch(&path, &rows)?;
        paths.push(path);
    }
    Ok(paths)
}

pub fn read_events_dir(root: &Path) -> Result<Vec<Event>> {
    let dir = root.join("cold/events");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("parquet") {
            out.extend(read_events_file(&path)?);
        }
    }
    out.sort_by(|a, b| (a.ts_ms, &a.session_id, a.seq).cmp(&(b.ts_ms, &b.session_id, b.seq)));
    Ok(out)
}

pub fn remove_partitions_older_than(root: &Path, cutoff_ms: u64) -> Result<u64> {
    let dir = root.join("cold/events");
    if !dir.exists() {
        return Ok(0);
    }
    let cutoff = partition_day(cutoff_ms)?;
    let mut removed = 0;
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem < cutoff.as_str() {
            std::fs::remove_file(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn write_batch(path: &Path, events: &[Event]) -> Result<()> {
    let batch = batch_from_events(events)?;
    let file = File::create(path)?;
    let props = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .set_key_value_metadata(Some(vec![KeyValue {
            key: "kaizen_schema_v".into(),
            value: Some(SCHEMA_VERSION.into()),
        }]))
        .build();
    let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

fn read_events_file(path: &Path) -> Result<Vec<Event>> {
    let file = File::open(path)?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file)?.build()?;
    let mut out = Vec::new();
    for batch in reader {
        out.extend(events_from_batch(&batch?)?);
    }
    Ok(out)
}

fn batch_from_events(events: &[Event]) -> Result<RecordBatch> {
    let payload = events
        .iter()
        .map(|e| serde_json::to_string(&e.payload).unwrap_or_else(|_| "null".into()))
        .collect::<Vec<_>>();
    let cols: Vec<(&str, ArrayRef)> = vec![
        (
            "session_id",
            Arc::new(StringArray::from(strs(events, |e| &e.session_id))),
        ),
        ("seq", Arc::new(UInt64Array::from(vals(events, |e| e.seq)))),
        (
            "ts_ms",
            Arc::new(UInt64Array::from(vals(events, |e| e.ts_ms))),
        ),
        (
            "ts_exact",
            Arc::new(BooleanArray::from(vals(events, |e| e.ts_exact))),
        ),
        (
            "kind",
            Arc::new(StringArray::from(strs(events, |e| kind(&e.kind)))),
        ),
        (
            "source",
            Arc::new(StringArray::from(strs(events, |e| source(&e.source)))),
        ),
        (
            "tool",
            Arc::new(StringArray::from(opts(events, |e| e.tool.clone()))),
        ),
        (
            "tool_call_id",
            Arc::new(StringArray::from(opts(events, |e| e.tool_call_id.clone()))),
        ),
        (
            "tokens_in",
            Arc::new(UInt32Array::from(opt_u32(events, |e| e.tokens_in))),
        ),
        (
            "tokens_out",
            Arc::new(UInt32Array::from(opt_u32(events, |e| e.tokens_out))),
        ),
        (
            "reasoning_tokens",
            Arc::new(UInt32Array::from(opt_u32(events, |e| e.reasoning_tokens))),
        ),
        (
            "cost_usd_e6",
            Arc::new(Int64Array::from(opt_i64(events, |e| e.cost_usd_e6))),
        ),
        ("payload", Arc::new(StringArray::from(payload))),
        (
            "stop_reason",
            Arc::new(StringArray::from(opts(events, |e| e.stop_reason.clone()))),
        ),
        (
            "latency_ms",
            Arc::new(UInt32Array::from(opt_u32(events, |e| e.latency_ms))),
        ),
        (
            "ttft_ms",
            Arc::new(UInt32Array::from(opt_u32(events, |e| e.ttft_ms))),
        ),
        (
            "retry_count",
            Arc::new(UInt16Array::from(opt_u16(events, |e| e.retry_count))),
        ),
        (
            "context_used_tokens",
            Arc::new(UInt32Array::from(opt_u32(events, |e| {
                e.context_used_tokens
            }))),
        ),
        (
            "context_max_tokens",
            Arc::new(UInt32Array::from(opt_u32(events, |e| e.context_max_tokens))),
        ),
        (
            "cache_creation_tokens",
            Arc::new(UInt32Array::from(opt_u32(events, |e| {
                e.cache_creation_tokens
            }))),
        ),
        (
            "cache_read_tokens",
            Arc::new(UInt32Array::from(opt_u32(events, |e| e.cache_read_tokens))),
        ),
        (
            "system_prompt_tokens",
            Arc::new(UInt32Array::from(opt_u32(events, |e| {
                e.system_prompt_tokens
            }))),
        ),
    ];
    Ok(RecordBatch::try_from_iter(cols)?)
}

fn events_from_batch(batch: &RecordBatch) -> Result<Vec<Event>> {
    let s = |i| str_col(batch, i);
    let u64c = |i| u64_col(batch, i);
    let out = (0..batch.num_rows())
        .map(|i| Event {
            session_id: s(0).value(i).into(),
            seq: u64c(1).value(i),
            ts_ms: u64c(2).value(i),
            ts_exact: bool_col(batch, 3).value(i),
            kind: kind_from(s(4).value(i)),
            source: source_from(s(5).value(i)),
            tool: opt_str(batch, 6, i),
            tool_call_id: opt_str(batch, 7, i),
            tokens_in: opt_u32_at(batch, 8, i),
            tokens_out: opt_u32_at(batch, 9, i),
            reasoning_tokens: opt_u32_at(batch, 10, i),
            cost_usd_e6: opt_i64_at(batch, 11, i),
            payload: serde_json::from_str(s(12).value(i)).unwrap_or(serde_json::Value::Null),
            stop_reason: opt_str(batch, 13, i),
            latency_ms: opt_u32_at(batch, 14, i),
            ttft_ms: opt_u32_at(batch, 15, i),
            retry_count: opt_u16_at(batch, 16, i),
            context_used_tokens: opt_u32_at(batch, 17, i),
            context_max_tokens: opt_u32_at(batch, 18, i),
            cache_creation_tokens: opt_u32_at(batch, 19, i),
            cache_read_tokens: opt_u32_at(batch, 20, i),
            system_prompt_tokens: opt_u32_at(batch, 21, i),
        })
        .collect();
    Ok(out)
}

pub fn partition_day(ts_ms: u64) -> Result<String> {
    let ts = OffsetDateTime::from_unix_timestamp((ts_ms / 1000) as i64)?;
    let text = ts.date().format(&Iso8601::DATE)?;
    Ok(text)
}

fn vals<T: Copy>(events: &[Event], f: impl Fn(&Event) -> T) -> Vec<T> {
    events.iter().map(f).collect()
}

fn strs(events: &[Event], f: impl Fn(&Event) -> &str) -> Vec<String> {
    events.iter().map(|e| f(e).to_string()).collect()
}

fn opts(events: &[Event], f: impl Fn(&Event) -> Option<String>) -> Vec<Option<String>> {
    events.iter().map(f).collect()
}

fn opt_u32(events: &[Event], f: impl Fn(&Event) -> Option<u32>) -> Vec<Option<u32>> {
    events.iter().map(f).collect()
}

fn opt_u16(events: &[Event], f: impl Fn(&Event) -> Option<u16>) -> Vec<Option<u16>> {
    events.iter().map(f).collect()
}

fn opt_i64(events: &[Event], f: impl Fn(&Event) -> Option<i64>) -> Vec<Option<i64>> {
    events.iter().map(f).collect()
}

fn str_col(batch: &RecordBatch, i: usize) -> &StringArray {
    batch.column(i).as_any().downcast_ref().unwrap()
}

fn u64_col(batch: &RecordBatch, i: usize) -> &UInt64Array {
    batch.column(i).as_any().downcast_ref().unwrap()
}

fn bool_col(batch: &RecordBatch, i: usize) -> &BooleanArray {
    batch.column(i).as_any().downcast_ref().unwrap()
}

fn opt_str(batch: &RecordBatch, col: usize, row: usize) -> Option<String> {
    let a = str_col(batch, col);
    (!a.is_null(row)).then(|| a.value(row).to_string())
}

fn opt_u32_at(batch: &RecordBatch, col: usize, row: usize) -> Option<u32> {
    let a = batch
        .column(col)
        .as_any()
        .downcast_ref::<UInt32Array>()
        .unwrap();
    (!a.is_null(row)).then(|| a.value(row))
}

fn opt_u16_at(batch: &RecordBatch, col: usize, row: usize) -> Option<u16> {
    let a = batch
        .column(col)
        .as_any()
        .downcast_ref::<UInt16Array>()
        .unwrap();
    (!a.is_null(row)).then(|| a.value(row))
}

fn opt_i64_at(batch: &RecordBatch, col: usize, row: usize) -> Option<i64> {
    let a = batch
        .column(col)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();
    (!a.is_null(row)).then(|| a.value(row))
}

fn kind(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::ToolCall => "ToolCall",
        EventKind::ToolResult => "ToolResult",
        EventKind::Message => "Message",
        EventKind::Error => "Error",
        EventKind::Cost => "Cost",
        EventKind::Hook => "Hook",
        EventKind::Lifecycle => "Lifecycle",
    }
}

fn source(source: &EventSource) -> &'static str {
    match source {
        EventSource::Tail => "Tail",
        EventSource::Hook => "Hook",
        EventSource::Proxy => "Proxy",
    }
}

fn kind_from(s: &str) -> EventKind {
    match s {
        "ToolCall" => EventKind::ToolCall,
        "ToolResult" => EventKind::ToolResult,
        "Error" => EventKind::Error,
        "Cost" => EventKind::Cost,
        "Hook" => EventKind::Hook,
        "Lifecycle" => EventKind::Lifecycle,
        _ => EventKind::Message,
    }
}

fn source_from(s: &str) -> EventSource {
    match s {
        "Hook" => EventSource::Hook,
        "Proxy" => EventSource::Proxy,
        _ => EventSource::Tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn write_and_read_daily_events() {
        let dir = tempfile::tempdir().unwrap();
        let event = Event {
            session_id: "s1".into(),
            seq: 0,
            ts_ms: 1_700_000_000_000,
            ts_exact: true,
            kind: EventKind::Message,
            source: EventSource::Hook,
            tool: None,
            tool_call_id: None,
            tokens_in: Some(10),
            tokens_out: None,
            reasoning_tokens: None,
            cost_usd_e6: Some(5),
            stop_reason: None,
            latency_ms: None,
            ttft_ms: None,
            retry_count: None,
            context_used_tokens: Some(12),
            context_max_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            system_prompt_tokens: None,
            payload: json!({"type": "note"}),
        };
        let paths = write_daily_events(dir.path(), std::slice::from_ref(&event)).unwrap();
        assert_eq!(paths.len(), 1);
        let rows = read_events_dir(dir.path()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, event.session_id);
        assert_eq!(rows[0].cost_usd_e6, Some(5));
        assert_eq!(rows[0].context_used_tokens, Some(12));
    }
}
