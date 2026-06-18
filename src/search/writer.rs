// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tantivy writer with batching and one-process file lock.

use crate::search::SearchDoc;
use crate::search::schema::{SearchFields, build_schema, event_key};
use anyhow::{Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tantivy::indexer::IndexWriterOptions;
use tantivy::schema::Term;
use tantivy::{Index, IndexWriter, TantivyDocument, doc};

const HEAP_BYTES: usize = 50_000_000;
const BATCH_DOCS: usize = 256;
const BATCH_SECS: u64 = 60;

pub struct PendingWriter {
    writer: IndexWriter<TantivyDocument>,
    fields: SearchFields,
    pending: usize,
    last_commit: Instant,
    _lock: File,
}

impl PendingWriter {
    pub fn open(root: &Path) -> Result<Self> {
        let dir = crate::core::paths::descendant_dir_for_write(root, Path::new("search"))?;
        reject_tree_symlinks(&dir)?;
        let lock = lock_file(&dir)?;
        let (schema, fields) = build_schema();
        let index = open_or_create(&dir, schema)?;
        let writer = index.writer_with_options(writer_options())?;
        Ok(Self {
            writer,
            fields,
            pending: 0,
            last_commit: Instant::now(),
            _lock: lock,
        })
    }

    pub fn add(&mut self, doc: &SearchDoc) -> Result<()> {
        if self.pending == 0 {
            self.last_commit = Instant::now();
        }
        self.writer.delete_term(Term::from_field_text(
            self.fields.event_key,
            &event_key(&doc.session_id, doc.seq),
        ));
        self.writer.add_document(self.tantivy_doc(doc))?;
        self.pending += 1;
        if self.should_commit() {
            self.commit()?;
        }
        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        if self.pending == 0 {
            return Ok(());
        }
        self.writer.commit()?;
        self.pending = 0;
        self.last_commit = Instant::now();
        Ok(())
    }

    fn should_commit(&self) -> bool {
        self.pending >= BATCH_DOCS || self.last_commit.elapsed() >= Duration::from_secs(BATCH_SECS)
    }

    fn tantivy_doc(&self, d: &SearchDoc) -> TantivyDocument {
        let mut doc = doc!(
            self.fields.session_id => d.session_id.clone(),
            self.fields.seq => d.seq as i64,
            self.fields.event_key => event_key(&d.session_id, d.seq),
            self.fields.ts_ms => d.ts_ms as i64,
            self.fields.agent => d.agent.clone(),
            self.fields.kind => d.kind.clone(),
            self.fields.text => d.text.clone(),
            self.fields.tokens_total => d.tokens_total,
        );
        d.paths
            .iter()
            .for_each(|p| doc.add_text(self.fields.path, p));
        d.skills
            .iter()
            .for_each(|s| doc.add_text(self.fields.skill, s));
        doc
    }
}

fn writer_options() -> IndexWriterOptions {
    IndexWriterOptions::builder()
        .memory_budget_per_thread(HEAP_BYTES)
        .num_worker_threads(1)
        .num_merge_threads(1)
        .build()
}

pub fn delete_sessions(root: &Path, ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut writer = PendingWriter::open(root)?;
    for id in ids {
        writer
            .writer
            .delete_term(Term::from_field_text(writer.fields.session_id, id));
    }
    writer.pending = ids.len();
    writer.commit()
}

pub fn index_dir(root: &Path) -> PathBuf {
    root.join("search")
}

fn lock_file(dir: &Path) -> Result<File> {
    let file = crate::core::safe_fs::read_write(&dir.join(".writer.lock"))?;
    file.lock().context("lock search writer")?;
    Ok(file)
}

fn reject_tree_symlinks(root: &Path) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "search index rejects symlink"
        );
        if metadata.is_dir() {
            reject_tree_symlinks(&path)?;
        }
    }
    Ok(())
}

fn open_or_create(dir: &Path, schema: tantivy::schema::Schema) -> Result<Index> {
    Ok(Index::open_in_dir(dir).or_else(|_| Index::create_in_dir(dir, schema))?)
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
