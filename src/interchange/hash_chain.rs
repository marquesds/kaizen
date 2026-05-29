// SPDX-License-Identifier: AGPL-3.0-or-later
//! Deterministic event hash-chain helpers.

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};

const HASH_PREFIX: &str = "blake3:";
const GENESIS: &str = "genesis";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashChainEvent {
    pub event_id: String,
    pub canonical_json: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashChainLink {
    pub event_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    pub event_hash: String,
    pub chain_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashChainError {
    Serialize(String),
    LengthMismatch {
        expected: usize,
        actual: usize,
    },
    LinkMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
}

impl HashChainEvent {
    pub fn from_json<T: Serialize>(event_id: String, event: &T) -> Result<Self, HashChainError> {
        Ok(Self {
            event_id,
            canonical_json: serde_json::to_vec(event)
                .map_err(|e| HashChainError::Serialize(e.to_string()))?,
        })
    }
}

pub fn compute_hash_chain(events: &[HashChainEvent]) -> Vec<HashChainLink> {
    events
        .iter()
        .scan(None, |prev, event| {
            let link = link_for(prev.clone(), event);
            *prev = Some(link.chain_hash.clone());
            Some(link)
        })
        .collect()
}

pub fn verify_hash_chain(
    events: &[HashChainEvent],
    links: &[HashChainLink],
) -> Result<(), HashChainError> {
    if events.len() != links.len() {
        return Err(HashChainError::LengthMismatch {
            expected: events.len(),
            actual: links.len(),
        });
    }
    first_mismatch(&compute_hash_chain(events), links).map_or(Ok(()), Err)
}

fn first_mismatch(expected: &[HashChainLink], actual: &[HashChainLink]) -> Option<HashChainError> {
    expected
        .iter()
        .zip(actual)
        .enumerate()
        .find_map(|(index, (a, b))| {
            (a != b).then(|| HashChainError::LinkMismatch {
                index,
                expected: a.chain_hash.clone(),
                actual: b.chain_hash.clone(),
            })
        })
}

fn link_for(prev_hash: Option<String>, event: &HashChainEvent) -> HashChainLink {
    let event_hash = hash_bytes(&event.canonical_json);
    HashChainLink {
        event_id: event.event_id.clone(),
        chain_hash: chain_hash(prev_hash.as_deref(), &event_hash),
        event_hash,
        prev_hash,
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = blake3::hash(bytes);
    format!("{HASH_PREFIX}{}", hex::encode(digest.as_bytes()))
}

fn chain_hash(prev_hash: Option<&str>, event_hash: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(prev_hash.unwrap_or(GENESIS).as_bytes());
    hasher.update(b"\n");
    hasher.update(event_hash.as_bytes());
    format!("{HASH_PREFIX}{}", hex::encode(hasher.finalize().as_bytes()))
}

impl Display for HashChainError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(err) => write!(f, "hash-chain serialize error: {err}"),
            Self::LengthMismatch { expected, actual } => {
                write!(
                    f,
                    "hash-chain length mismatch: expected {expected}, got {actual}"
                )
            }
            Self::LinkMismatch { index, .. } => write!(f, "hash-chain link mismatch at {index}"),
        }
    }
}

impl Error for HashChainError {}
