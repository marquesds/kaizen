// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workspace-local Tantivy search index.

pub mod extract;
pub mod reader;
pub mod reindex;
pub mod schema;
pub mod writer;

pub use extract::{SearchDoc, extract_doc, kind_label, tokens_total};
pub use reader::{SearchHit, SearchQuery, search};
pub use reindex::reindex_workspace;
pub use writer::{PendingWriter, delete_sessions};
