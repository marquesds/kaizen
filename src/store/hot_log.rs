// SPDX-License-Identifier: AGPL-3.0-or-later
//! HOT append log: mmap replay, rkyv records, redb offset indexes.

use crate::core::event::{Event, EventKind, EventSource};
use anyhow::{Context, Result, anyhow};
use crc32fast::Hasher;
use memmap2::Mmap;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

const MAGIC: u64 = 0x4b41495a454e484f;
const VERSION: u32 = 1;
const HEADER_LEN: u64 = 12;
const SESSIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("sessions");
const SEQ_IDX: TableDefinition<&str, u64> = TableDefinition::new("seq_idx");

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionMeta {
    pub first_offset: u64,
    pub last_offset: u64,
    pub last_seq: u64,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize)]
struct HotEventRecord {
    session_id: String,
    seq: u64,
    ts_ms: u64,
    ts_exact: bool,
    kind: String,
    source: String,
    tool: Option<String>,
    tool_call_id: Option<String>,
    tokens_in: Option<u32>,
    tokens_out: Option<u32>,
    reasoning_tokens: Option<u32>,
    cost_usd_e6: Option<i64>,
    stop_reason: Option<String>,
    latency_ms: Option<u32>,
    ttft_ms: Option<u32>,
    retry_count: Option<u16>,
    context_used_tokens: Option<u32>,
    context_max_tokens: Option<u32>,
    cache_creation_tokens: Option<u32>,
    cache_read_tokens: Option<u32>,
    system_prompt_tokens: Option<u32>,
    payload_json: String,
}

pub struct HotLog {
    file: File,
    index: Database,
    pending_index: Vec<(String, u64, u64)>,
    bytes_since_sync: u64,
}

impl HotLog {
    pub fn open(root: &Path) -> Result<Self> {
        let dir = root.join("hot");
        std::fs::create_dir_all(&dir)?;
        let log_path = dir.join("log.bin");
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&log_path)?;
        ensure_header(&mut file)?;
        let index = Database::create(dir.join("index.redb"))?;
        Ok(Self {
            file,
            index,
            pending_index: Vec::new(),
            bytes_since_sync: 0,
        })
    }

    pub fn append(&mut self, event: &Event) -> Result<u64> {
        let offset = self.file.seek(SeekFrom::End(0))?;
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&HotEventRecord::from(event))?;
        let crc = crc32(&bytes);
        self.file.write_all(&(bytes.len() as u32).to_le_bytes())?;
        self.file.write_all(&bytes)?;
        self.file.write_all(&crc.to_le_bytes())?;
        self.bytes_since_sync += 8 + bytes.len() as u64;
        self.pending_index
            .push((event.session_id.clone(), event.seq, offset));
        if self.bytes_since_sync >= 4096 || self.pending_index.len() >= 128 {
            self.flush()?;
        }
        Ok(offset)
    }

    pub fn flush(&mut self) -> Result<()> {
        if !self.pending_index.is_empty() {
            self.flush_index()?;
        }
        if self.bytes_since_sync > 0 {
            self.file.sync_data()?;
            self.bytes_since_sync = 0;
        }
        Ok(())
    }

    pub fn replay(root: &Path) -> Result<Vec<(u64, Event)>> {
        let path = root.join("hot/log.bin");
        let file =
            File::open(&path).with_context(|| format!("open hot log: {}", path.display()))?;
        if file.metadata()?.len() <= HEADER_LEN {
            return Ok(Vec::new());
        }
        // SAFETY: read-only map over stable file handle; records validated by len + CRC.
        let mmap = unsafe { Mmap::map(&file)? };
        validate_header(&mmap)?;
        read_records(&mmap)
    }

    pub fn offset_for(root: &Path, session_id: &str, seq: u64) -> Result<Option<u64>> {
        let db = Database::create(root.join("hot/index.redb"))?;
        let tx = db.begin_read()?;
        let table = tx.open_table(SEQ_IDX)?;
        Ok(table
            .get(seq_key(session_id, seq).as_str())?
            .map(|v| v.value()))
    }

    fn flush_index(&mut self) -> Result<()> {
        let tx = self.index.begin_write()?;
        {
            let mut sessions = tx.open_table(SESSIONS)?;
            for (session_id, rows) in pending_by_session(&self.pending_index) {
                let prior = sessions
                    .get(session_id.as_str())?
                    .map(|v| serde_json::from_slice::<SessionMeta>(v.value()))
                    .transpose()?;
                let first_offset = prior
                    .map(|m| m.first_offset)
                    .unwrap_or_else(|| rows.first().map(|(_, o)| *o).unwrap_or(0));
                let (last_seq, last_offset) = rows.last().copied().unwrap_or((0, first_offset));
                let bytes = serde_json::to_vec(&SessionMeta {
                    first_offset,
                    last_offset,
                    last_seq,
                })?;
                sessions.insert(session_id.as_str(), bytes.as_slice())?;
            }
        }
        {
            let mut seq_idx = tx.open_table(SEQ_IDX)?;
            for (session_id, seq, offset) in &self.pending_index {
                seq_idx.insert(seq_key(session_id, *seq).as_str(), *offset)?;
            }
        }
        tx.commit()?;
        self.pending_index.clear();
        Ok(())
    }
}

impl Drop for HotLog {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn ensure_header(file: &mut File) -> Result<()> {
    if file.metadata()?.len() == 0 {
        file.write_all(&MAGIC.to_le_bytes())?;
        file.write_all(&VERSION.to_le_bytes())?;
        file.sync_data()?;
    }
    Ok(())
}

fn validate_header(bytes: &[u8]) -> Result<()> {
    let magic = u64::from_le_bytes(bytes[0..8].try_into()?);
    let version = u32::from_le_bytes(bytes[8..12].try_into()?);
    if magic != MAGIC || version != VERSION {
        return Err(anyhow!("invalid hot log header"));
    }
    Ok(())
}

fn read_records(bytes: &[u8]) -> Result<Vec<(u64, Event)>> {
    let mut out = Vec::new();
    let mut pos = HEADER_LEN as usize;
    while pos + 8 <= bytes.len() {
        let offset = pos as u64;
        let len = u32::from_le_bytes(bytes[pos..pos + 4].try_into()?) as usize;
        let start = pos + 4;
        let end = start + len;
        if end + 4 > bytes.len() {
            break;
        }
        let expected = u32::from_le_bytes(bytes[end..end + 4].try_into()?);
        if crc32(&bytes[start..end]) != expected {
            break;
        }
        let _ = rkyv::access::<ArchivedHotEventRecord, rkyv::rancor::Error>(&bytes[start..end])?;
        let rec = rkyv::from_bytes::<HotEventRecord, rkyv::rancor::Error>(&bytes[start..end])?;
        out.push((offset, rec.into_event()?));
        pos = end + 4;
    }
    Ok(out)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize()
}

fn seq_key(session_id: &str, seq: u64) -> String {
    format!("{session_id}\0{seq:020}")
}

fn pending_by_session(rows: &[(String, u64, u64)]) -> Vec<(String, Vec<(u64, u64)>)> {
    let mut grouped = std::collections::BTreeMap::<String, Vec<(u64, u64)>>::new();
    for (session_id, seq, offset) in rows {
        grouped
            .entry(session_id.clone())
            .or_default()
            .push((*seq, *offset));
    }
    grouped
        .into_iter()
        .map(|(session_id, mut rows)| {
            rows.sort_by_key(|(seq, _)| *seq);
            (session_id, rows)
        })
        .collect()
}

impl From<&Event> for HotEventRecord {
    fn from(e: &Event) -> Self {
        Self {
            session_id: e.session_id.clone(),
            seq: e.seq,
            ts_ms: e.ts_ms,
            ts_exact: e.ts_exact,
            kind: format!("{:?}", e.kind),
            source: format!("{:?}", e.source),
            tool: e.tool.clone(),
            tool_call_id: e.tool_call_id.clone(),
            tokens_in: e.tokens_in,
            tokens_out: e.tokens_out,
            reasoning_tokens: e.reasoning_tokens,
            cost_usd_e6: e.cost_usd_e6,
            stop_reason: e.stop_reason.clone(),
            latency_ms: e.latency_ms,
            ttft_ms: e.ttft_ms,
            retry_count: e.retry_count,
            context_used_tokens: e.context_used_tokens,
            context_max_tokens: e.context_max_tokens,
            cache_creation_tokens: e.cache_creation_tokens,
            cache_read_tokens: e.cache_read_tokens,
            system_prompt_tokens: e.system_prompt_tokens,
            payload_json: serde_json::to_string(&e.payload).unwrap_or_else(|_| "null".into()),
        }
    }
}

impl HotEventRecord {
    fn into_event(self) -> Result<Event> {
        Ok(Event {
            session_id: self.session_id,
            seq: self.seq,
            ts_ms: self.ts_ms,
            ts_exact: self.ts_exact,
            kind: kind_from_str(&self.kind),
            source: source_from_str(&self.source),
            tool: self.tool,
            tool_call_id: self.tool_call_id,
            tokens_in: self.tokens_in,
            tokens_out: self.tokens_out,
            reasoning_tokens: self.reasoning_tokens,
            cost_usd_e6: self.cost_usd_e6,
            stop_reason: self.stop_reason,
            latency_ms: self.latency_ms,
            ttft_ms: self.ttft_ms,
            retry_count: self.retry_count,
            context_used_tokens: self.context_used_tokens,
            context_max_tokens: self.context_max_tokens,
            cache_creation_tokens: self.cache_creation_tokens,
            cache_read_tokens: self.cache_read_tokens,
            system_prompt_tokens: self.system_prompt_tokens,
            payload: serde_json::from_str(&self.payload_json)?,
        })
    }
}

fn kind_from_str(s: &str) -> EventKind {
    match s {
        "ToolCall" => EventKind::ToolCall,
        "ToolResult" => EventKind::ToolResult,
        "Message" => EventKind::Message,
        "Error" => EventKind::Error,
        "Cost" => EventKind::Cost,
        "Hook" => EventKind::Hook,
        "Lifecycle" => EventKind::Lifecycle,
        _ => EventKind::Message,
    }
}

fn source_from_str(s: &str) -> EventSource {
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
    fn append_replay_and_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let event = Event {
            session_id: "s1".into(),
            seq: 7,
            ts_ms: 1_700_000_000_000,
            ts_exact: true,
            kind: EventKind::ToolCall,
            source: EventSource::Tail,
            tool: Some("bash".into()),
            tool_call_id: Some("c1".into()),
            tokens_in: Some(1),
            tokens_out: Some(2),
            reasoning_tokens: Some(3),
            cost_usd_e6: Some(4),
            stop_reason: None,
            latency_ms: None,
            ttft_ms: None,
            retry_count: None,
            context_used_tokens: None,
            context_max_tokens: None,
            cache_creation_tokens: None,
            cache_read_tokens: None,
            system_prompt_tokens: None,
            payload: json!({"ok": true}),
        };
        let mut log = HotLog::open(root).unwrap();
        let offset = log.append(&event).unwrap();
        drop(log);
        assert_eq!(HotLog::offset_for(root, "s1", 7).unwrap(), Some(offset));
        let rows = HotLog::replay(root).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1.session_id, "s1");
        assert_eq!(rows[0].1.seq, 7);
    }
}
