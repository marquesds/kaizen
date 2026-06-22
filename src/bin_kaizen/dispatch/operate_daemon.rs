use crate::bin_kaizen::args::*;
use anyhow::{Result, anyhow};
use std::process::Command;

pub(super) fn daemon(cmd: DaemonCommand) -> anyhow::Result<()> {
    match cmd {
        DaemonCommand::Start { background } => daemon_start(background),
        DaemonCommand::Stop => {
            println!("{}", kaizen::daemon::stop()?);
            Ok(())
        }
        DaemonCommand::Restart => daemon_restart(),
        DaemonCommand::Status => daemon_status(),
    }
}

pub(super) fn open_web(no_browser: bool) -> Result<()> {
    anyhow::ensure!(
        kaizen::daemon::enabled(),
        "dashboard requires the daemon; remove --no-daemon"
    );
    let started = kaizen::daemon::start_background()?;
    let web = started
        .web
        .ok_or_else(|| anyhow!("dashboard unavailable"))?;
    println!("Kaizen dashboard: {}", web.url);
    if !no_browser && let Err(err) = launch_browser(&web.url) {
        eprintln!("Could not open browser: {err}");
    }
    Ok(())
}

fn launch_browser(url: &str) -> Result<()> {
    let status = browser_command(url)?.status()?;
    anyhow::ensure!(status.success(), "browser launcher exited with {status}");
    Ok(())
}

#[cfg(target_os = "macos")]
fn browser_command(url: &str) -> Result<Command> {
    let mut command = Command::new("open");
    command.arg(url);
    Ok(command)
}

#[cfg(target_os = "linux")]
fn browser_command(url: &str) -> Result<Command> {
    let mut command = Command::new("xdg-open");
    command.arg(url);
    Ok(command)
}

#[cfg(target_os = "windows")]
fn browser_command(url: &str) -> Result<Command> {
    let mut command = Command::new("cmd");
    command.args(["/C", "start", "", url]);
    Ok(command)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn browser_command(_url: &str) -> Result<Command> {
    Err(anyhow!("browser launch unsupported on this platform"))
}

fn daemon_start(background: bool) -> anyhow::Result<()> {
    if !background {
        return kaizen::daemon::start_foreground();
    }
    let started = kaizen::daemon::start_background()?;
    for line in background_start_lines(&started) {
        println!("{line}");
    }
    Ok(())
}

fn daemon_restart() -> anyhow::Result<()> {
    let started = kaizen::daemon::restart_background()?;
    for line in background_start_lines(&started) {
        println!("{line}");
    }
    Ok(())
}

fn background_start_lines(started: &kaizen::daemon::BackgroundStart) -> Vec<String> {
    let lines = [
        format!("daemon {}", background_state(started)),
        format!("pid: {}", started.pid),
        format!("socket: {}", started.paths.sock.display()),
        format!("log: {}", started.paths.log.display()),
    ];
    lines.into_iter().chain(web_line(started)).collect()
}

fn background_state(started: &kaizen::daemon::BackgroundStart) -> &'static str {
    match started.already_running {
        true => "already running",
        false => "started",
    }
}

fn web_line(started: &kaizen::daemon::BackgroundStart) -> Option<String> {
    started.web.as_ref().map(|web| format!("web: {}", web.url))
}

fn daemon_status() -> anyhow::Result<()> {
    match kaizen::daemon::status_outcome()? {
        kaizen::daemon::DaemonStatusOutcome::Running(st) => print_running_daemon(st),
        kaizen::daemon::DaemonStatusOutcome::Stopped { socket } => {
            println!("status: stopped");
            println!("socket: {}", socket.display());
        }
    }
    Ok(())
}

fn print_running_daemon(st: kaizen::ipc::DaemonStatus) {
    println!("status: running");
    println!("pid: {}", st.pid);
    println!("uptime_ms: {}", st.uptime_ms);
    println!("queue_depth: {}", st.queue_depth);
    println!(
        "last_error: {}",
        st.last_error.unwrap_or_else(|| "-".to_string())
    );
    if let Some(web) = st.web {
        println!("web: {}", web.url);
    }
    for capture in st.capture {
        print_capture(capture);
    }
}

fn print_capture(capture: kaizen::ipc::CaptureStatus) {
    println!("capture: {}", capture.workspace);
    println!("  deep: {}", capture.deep);
    println!("  hooks: {}", capture.hooks.len());
    println!("  watchers: {}", capture.watchers.len());
    println!("  proxies: {}", capture.proxies.len());
    if !capture.errors.is_empty() {
        println!("  errors: {}", capture.errors.join("; "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_start_output_includes_web_url() {
        let started = background_start();
        assert!(
            background_start_lines(&started)
                .contains(&"web: http://127.0.0.1:7878/?token=t".to_string())
        );
    }

    fn background_start() -> kaizen::daemon::BackgroundStart {
        kaizen::daemon::BackgroundStart {
            pid: 42,
            paths: runtime_paths(),
            already_running: false,
            web: Some(web_endpoint()),
        }
    }

    fn web_endpoint() -> kaizen::ipc::WebEndpoint {
        kaizen::ipc::WebEndpoint {
            listen: "127.0.0.1:7878".to_string(),
            url: "http://127.0.0.1:7878/?token=t".to_string(),
            token: "t".to_string(),
        }
    }

    fn runtime_paths() -> kaizen::daemon::RuntimePaths {
        kaizen::daemon::RuntimePaths {
            dir: "/tmp/k".into(),
            pid: "/tmp/k/daemon.pid".into(),
            sock: "/tmp/k/daemon.sock".into(),
            log: "/tmp/k/daemon.log".into(),
            token: "/tmp/k/web_token.hex".into(),
        }
    }
}
