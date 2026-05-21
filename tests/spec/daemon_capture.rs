// SPDX-License-Identifier: AGPL-3.0-or-later
use kaizen::ipc::{
    CaptureComponent, CaptureComponentStatus, CaptureStatus, DaemonRequest, DaemonResponse,
    ProxyEndpoint,
};
use kaizen::shell::init::{InitOptions, init_text_with_options};

#[test]
fn daemon_capture_ipc_round_trips() -> anyhow::Result<()> {
    let req = DaemonRequest::EnsureWorkspaceCapture {
        workspace: "/repo".into(),
        deep: true,
    };
    let raw = serde_json::to_value(&req)?;
    assert_eq!(raw["type"], "ensure_workspace_capture");
    match serde_json::from_value::<DaemonRequest>(raw)? {
        DaemonRequest::EnsureWorkspaceCapture { workspace, deep } => {
            assert_eq!(workspace, "/repo");
            assert!(deep);
        }
        other => panic!("unexpected request: {other:?}"),
    }

    let status = CaptureStatus {
        workspace: "/repo".into(),
        deep: true,
        hooks: vec![CaptureComponent {
            name: "claude".into(),
            status: CaptureComponentStatus::Ready,
            detail: Some("configured".into()),
        }],
        watchers: vec![CaptureComponent {
            name: "transcript-scanner".into(),
            status: CaptureComponentStatus::Ready,
            detail: None,
        }],
        proxies: vec![ProxyEndpoint {
            provider: "openai".into(),
            listen: "127.0.0.1:3847".into(),
            base_url: "http://127.0.0.1:3847".into(),
            v1_base_url: Some("http://127.0.0.1:3847/v1".into()),
        }],
        errors: Vec::new(),
    };
    let resp = DaemonResponse::CaptureStatus(Box::new(status));
    let raw = serde_json::to_value(&resp)?;
    assert_eq!(raw["type"], "capture_status");
    assert!(serde_json::from_value::<DaemonResponse>(raw).is_ok());
    Ok(())
}

#[test]
fn observe_env_uses_daemon_proxy_endpoint() {
    let endpoint = ProxyEndpoint {
        provider: "openai".into(),
        listen: "127.0.0.1:4848".into(),
        base_url: "http://127.0.0.1:4848".into(),
        v1_base_url: Some("http://127.0.0.1:4848/v1".into()),
    };
    let env = kaizen::shell::observe::observed_session_env("codex", &endpoint, "s1");
    assert!(env.contains(&("KAIZEN_SESSION_KEY".into(), "s1".into())));
    assert!(env.contains(&("X_KAIZEN_SESSION".into(), "s1".into())));
    assert!(env.contains(&("OPENAI_BASE_URL".into(), "http://127.0.0.1:4848/v1".into())));
    assert!(!env.iter().any(|(key, _)| key == "ANTHROPIC_BASE_URL"));
}

#[test]
fn init_capture_respects_no_daemon_mode() -> anyhow::Result<()> {
    let _guard = env_lock().lock().unwrap();
    let old_home = std::env::var_os("HOME");
    let old_daemon = std::env::var_os("KAIZEN_DAEMON");
    let tmp = tempfile::tempdir()?;
    let ws = tmp.path().join("repo");
    std::fs::create_dir_all(&ws)?;

    unsafe {
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("KAIZEN_DAEMON", "0");
    }
    let out = init_text_with_options(
        Some(&ws),
        InitOptions {
            deep: true,
            start_capture: true,
        },
    )?;
    restore_env("HOME", old_home);
    restore_env("KAIZEN_DAEMON", old_daemon);

    assert!(out.contains("skipped  daemon capture"));
    assert!(out.contains("kaizen init complete"));
    Ok(())
}

fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(v) => unsafe { std::env::set_var(key, v) },
        None => unsafe { std::env::remove_var(key) },
    }
}
