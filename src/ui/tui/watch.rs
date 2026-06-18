// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::Result;
use crossterm::event::{self as cxev, Event as CxEvent, KeyEvent, KeyEventKind};
use notify::{EventKind as NotifyEventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep_until};

pub(super) const WAL_REFRESH_COALESCE_MS: u64 = 100;

pub(super) fn spawn_key_reader(stop: Arc<AtomicBool>) -> mpsc::UnboundedReceiver<KeyEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::task::spawn_blocking(move || read_keys(stop, tx));
    rx
}

fn read_keys(stop: Arc<AtomicBool>, tx: mpsc::UnboundedSender<KeyEvent>) {
    while !stop.load(Ordering::Acquire) {
        match cxev::poll(Duration::from_millis(250)) {
            Ok(true) if read_key(&tx).is_err() => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

fn read_key(tx: &mpsc::UnboundedSender<KeyEvent>) -> Result<(), ()> {
    match cxev::read() {
        Ok(CxEvent::Key(key)) if key.kind == KeyEventKind::Press => tx.send(key).map_err(|_| ()),
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }
}

pub(super) fn spawn_wal_watcher(
    wal_path: &Path,
    dirty: Arc<AtomicBool>,
) -> Result<(RecommendedWatcher, mpsc::UnboundedReceiver<()>)> {
    let (tx, rx) = mpsc::unbounded_channel();
    let watched_wal = wal_path.to_path_buf();
    let callback_wal = watched_wal.clone();
    let mut watcher = RecommendedWatcher::new(
        move |result: notify::Result<notify::Event>| {
            if wal_changed(result, &callback_wal) {
                dirty.store(true, Ordering::Release);
                let _ = tx.send(());
            }
        },
        notify::Config::default(),
    )?;
    watcher.watch(
        watched_wal.parent().unwrap_or_else(|| Path::new(".")),
        RecursiveMode::NonRecursive,
    )?;
    Ok((watcher, rx))
}

fn wal_changed(result: notify::Result<notify::Event>, wal_path: &Path) -> bool {
    result.is_ok_and(|event| {
        !matches!(event.kind, NotifyEventKind::Access(_))
            && event.paths.iter().any(|path| path == wal_path)
    })
}

pub(super) async fn wait_for_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}
