// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon IPC protocol. JSON control frames now; payload marker leaves room for Arrow IPC batches.

use crate::core::event::{Event, SessionRecord};
use crate::store::{SessionFilter, SessionPage, SpanNode};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTO_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Tui,
    Cli,
    Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    pub proto_version: u32,
    pub client: ClientKind,
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    pub proto_version: u32,
    pub daemon_version: String,
    pub workspaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub pid: u32,
    pub uptime_ms: u64,
    pub queue_depth: usize,
    pub last_error: Option<String>,
    #[serde(default)]
    pub capture: Vec<CaptureStatus>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureComponentStatus {
    Ready,
    Partial,
    Unsupported,
    Error,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CaptureComponent {
    pub name: String,
    pub status: CaptureComponentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProxyEndpoint {
    pub provider: String,
    pub listen: String,
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v1_base_url: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CaptureStatus {
    pub workspace: String,
    pub deep: bool,
    pub hooks: Vec<CaptureComponent>,
    pub watchers: Vec<CaptureComponent>,
    pub proxies: Vec<ProxyEndpoint>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservedSession {
    pub session: String,
    pub proxies: Vec<ProxyEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDetail {
    pub session: Option<SessionRecord>,
    pub events: Vec<Event>,
    pub spans: Vec<SpanNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    Hello(ClientHello),
    Status,
    Stop,
    ListSessions {
        workspace: String,
        offset: usize,
        limit: usize,
        filter: SessionFilter,
    },
    GetSessionDetail {
        id: String,
        workspace: Option<String>,
    },
    IngestHook {
        source: crate::shell::ingest::IngestSource,
        payload: String,
        workspace: Option<String>,
    },
    EnsureWorkspaceCapture {
        workspace: String,
        deep: bool,
    },
    EnsureProxy {
        workspace: String,
        provider: String,
    },
    BeginObservedSession {
        workspace: String,
        agent: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    Hello(ServerHello),
    Status(DaemonStatus),
    Sessions(SessionPage),
    Detail(Box<SessionDetail>),
    Ack {
        message: String,
    },
    CaptureStatus(Box<CaptureStatus>),
    ProxyEndpoint(ProxyEndpoint),
    ObservedSession(ObservedSession),
    Error {
        message: String,
        supported_min: Option<u32>,
        supported_max: Option<u32>,
    },
}

pub async fn read_frame<T, R>(reader: &mut R) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
    R: AsyncRead + Unpin,
{
    let len = reader.read_u32().await? as usize;
    let mut buf = vec![0_u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(serde_json::from_slice(&buf)?)
}

pub async fn write_frame<T, W>(writer: &mut W, value: &T) -> anyhow::Result<()>
where
    T: Serialize,
    W: AsyncWrite + Unpin,
{
    let buf = serde_json::to_vec(value)?;
    writer.write_u32(buf.len() as u32).await?;
    writer.write_all(&buf).await?;
    writer.flush().await?;
    Ok(())
}
