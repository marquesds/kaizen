use crate::bin_kaizen::args::*;

pub(super) fn daemon(cmd: DaemonCommand) -> anyhow::Result<()> {
    match cmd {
        DaemonCommand::Start { background } => daemon_start(background),
        DaemonCommand::Stop => {
            println!("{}", kaizen::daemon::stop()?);
            Ok(())
        }
        DaemonCommand::Status => daemon_status(),
    }
}

fn daemon_start(background: bool) -> anyhow::Result<()> {
    if !background {
        return kaizen::daemon::start_foreground();
    }
    let started = kaizen::daemon::start_background()?;
    println!(
        "daemon {}",
        if started.already_running {
            "already running"
        } else {
            "started"
        }
    );
    println!("pid: {}", started.pid);
    println!("socket: {}", started.paths.sock.display());
    println!("log: {}", started.paths.log.display());
    if let Ok(status) = kaizen::daemon::try_status()
        && let Some(web) = status.web
    {
        println!("web: {}", web.url);
    }
    Ok(())
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
