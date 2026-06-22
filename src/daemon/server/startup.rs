// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daemon startup ordering: restore capture before socket and Web readiness.

use super::*;

pub(super) async fn start(paths: &RuntimePaths) -> Result<(File, UnixListener, ServerState)> {
    let (pid_lock, supervisor) = prepare_runtime(paths).await?;
    let (listener, state) = bind_services(paths, supervisor).await?;
    Ok((pid_lock, listener, state))
}

async fn prepare_runtime(paths: &RuntimePaths) -> Result<(File, Supervisor)> {
    std::fs::create_dir_all(&paths.dir)?;
    let pid_lock = lock_pid(paths)?;
    remove_stale_socket(&paths.sock)?;
    let supervisor = Supervisor::default();
    supervisor.restore_registered().await?;
    Ok((pid_lock, supervisor))
}

async fn bind_services(
    paths: &RuntimePaths,
    supervisor: Supervisor,
) -> Result<(UnixListener, ServerState)> {
    let listener = bind_listener(paths)?;
    let (web, _web_task) = crate::web::start(&paths.token).await?;
    let (state, rx) = new_state(supervisor, web);
    spawn_worker(rx, state.queue_depth.clone(), state.last_error.clone());
    Ok((listener, state))
}

fn bind_listener(paths: &RuntimePaths) -> Result<UnixListener> {
    let listener = UnixListener::bind(&paths.sock)
        .with_context(|| format!("bind daemon socket: {}", paths.sock.display()))?;
    set_socket_private(&paths.sock)?;
    Ok(listener)
}

fn new_state(supervisor: Supervisor, web: WebEndpoint) -> (ServerState, mpsc::Receiver<Job>) {
    let (tx, rx) = mpsc::channel(128);
    let state = ServerState {
        started: Instant::now(),
        queue_depth: Arc::new(AtomicUsize::new(0)),
        last_error: Arc::new(Mutex::new(None)),
        supervisor,
        tx,
        web,
    };
    (state, rx)
}
