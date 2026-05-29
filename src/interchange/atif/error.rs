// SPDX-License-Identifier: AGPL-3.0-or-later

use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtifImportError {
    UnsupportedFormat(String),
    UnsupportedVersion(u16),
    SessionMismatch {
        event_id: String,
        session_id: String,
    },
}

impl Display for AtifImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedFormat(v) => write!(f, "unsupported ATIF format: {v}"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported ATIF version: {v}"),
            Self::SessionMismatch {
                event_id,
                session_id,
            } => {
                write!(
                    f,
                    "event {event_id} does not belong to session {session_id}"
                )
            }
        }
    }
}

impl Error for AtifImportError {}
