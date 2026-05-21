// SPDX-License-Identifier: AGPL-3.0-or-later
//! Long-lived daemon capture tasks: transcript scanner loops and LLM proxy tasks.

use super::capture_status::{base_status, component};
use super::proxy_task::{normalize_provider, providers_for_agent, start_proxy};
use super::scanner_task::scanner_loop;
use crate::ipc::{
    CaptureComponent, CaptureComponentStatus, CaptureStatus, ObservedSession, ProxyEndpoint,
};
use anyhow::Result;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Clone, Default)]
pub(super) struct Supervisor {
    inner: Arc<Mutex<SupervisorState>>,
}

#[derive(Default)]
struct SupervisorState {
    captures: BTreeMap<String, CaptureStatus>,
    proxies: HashMap<ProxyKey, ProxyHandle>,
    scanners: HashSet<String>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct ProxyKey {
    workspace: String,
    provider: String,
}

struct ProxyHandle {
    endpoint: ProxyEndpoint,
    task: JoinHandle<()>,
}

impl Supervisor {
    pub(super) fn statuses(&self) -> Vec<CaptureStatus> {
        self.inner
            .lock()
            .map(|st| st.captures.values().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) async fn ensure_capture(&self, workspace: String, deep: bool) -> CaptureStatus {
        let ws = workspace_path(&workspace);
        let mut status = base_status(&ws, deep);
        status.watchers.push(self.ensure_scanner(ws.clone()));
        if deep {
            self.add_deep_capture(&ws, &mut status).await;
        }
        self.remember_capture(status.clone());
        status
    }

    pub(super) async fn ensure_proxy(
        &self,
        workspace: String,
        provider: String,
    ) -> Result<ProxyEndpoint> {
        let ws = workspace_path(&workspace);
        let key = ProxyKey {
            workspace: ws.to_string_lossy().to_string(),
            provider: normalize_provider(&provider)?,
        };
        if let Some(endpoint) = self.existing_proxy(&key) {
            return Ok(endpoint);
        }
        let (endpoint, task) = start_proxy(&ws, &key.provider).await?;
        self.remember_proxy(key, endpoint.clone(), task);
        Ok(endpoint)
    }

    pub(super) async fn begin_session(
        &self,
        workspace: String,
        agent: String,
    ) -> Result<ObservedSession> {
        self.ensure_capture(workspace.clone(), false).await;
        let mut proxies = Vec::new();
        for provider in providers_for_agent(&agent) {
            proxies.push(
                self.ensure_proxy(workspace.clone(), provider.into())
                    .await?,
            );
        }
        Ok(ObservedSession {
            session: format!("observe-{}", Uuid::now_v7()),
            proxies,
        })
    }

    async fn add_deep_capture(&self, ws: &Path, status: &mut CaptureStatus) {
        for provider in ["anthropic", "openai"] {
            match self
                .ensure_proxy(status.workspace.clone(), provider.into())
                .await
            {
                Ok(endpoint) => status.proxies.push(endpoint),
                Err(err) => status.errors.push(format!("{provider} proxy: {err:#}")),
            }
        }
        status.hooks.push(component(
            "external-agent-routing",
            CaptureComponentStatus::Partial,
            Some(format!("manual opt-in still required for {}", ws.display())),
        ));
    }

    fn ensure_scanner(&self, ws: PathBuf) -> CaptureComponent {
        let key = ws.to_string_lossy().to_string();
        if self
            .inner
            .lock()
            .map(|mut st| st.scanners.insert(key))
            .unwrap_or(false)
        {
            tokio::spawn(scanner_loop(ws));
        }
        component("transcript-scanner", CaptureComponentStatus::Ready, None)
    }

    fn existing_proxy(&self, key: &ProxyKey) -> Option<ProxyEndpoint> {
        let mut st = self.inner.lock().ok()?;
        if st.proxies.get(key).is_some_and(|h| h.task.is_finished()) {
            st.proxies.remove(key);
        }
        st.proxies.get(key).map(|h| h.endpoint.clone())
    }

    fn remember_proxy(&self, key: ProxyKey, endpoint: ProxyEndpoint, task: JoinHandle<()>) {
        if let Ok(mut st) = self.inner.lock() {
            st.proxies.insert(
                key.clone(),
                ProxyHandle {
                    endpoint: endpoint.clone(),
                    task,
                },
            );
            if let Some(capture) = st.captures.get_mut(&key.workspace) {
                capture.proxies.retain(|p| p.provider != key.provider);
                capture.proxies.push(endpoint);
            }
        }
    }

    fn remember_capture(&self, mut status: CaptureStatus) {
        if let Ok(mut st) = self.inner.lock() {
            for handle in st
                .proxies
                .iter()
                .filter(|(key, _)| key.workspace == status.workspace)
                .map(|(_, handle)| handle)
            {
                if !status
                    .proxies
                    .iter()
                    .any(|proxy| proxy.provider == handle.endpoint.provider)
                {
                    status.proxies.push(handle.endpoint.clone());
                }
            }
            st.captures.insert(status.workspace.clone(), status);
        }
    }
}

fn workspace_path(workspace: &str) -> PathBuf {
    crate::core::paths::canonical(&PathBuf::from(workspace))
}
