// SPDX-License-Identifier: AGPL-3.0-or-later
#[cfg(unix)]
mod unix {
    use std::process::Command;

    #[test]
    fn background_daemon_starts_in_its_own_session() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join(".kaizen");
        let bin = env!("CARGO_BIN_EXE_kaizen");
        let start = daemon(bin, &home, ["daemon", "start", "--background"]);
        assert!(start.status.success(), "{}", stderr(&start));
        let pid = parse_pid(&start.stdout);
        let sid = session_id(pid);
        let _ = daemon(bin, &home, ["daemon", "stop"]);
        assert_eq!(sid, pid, "daemon pid={pid} inherited session {sid}");
    }

    fn daemon<const N: usize>(
        bin: &str,
        home: &std::path::Path,
        args: [&str; N],
    ) -> std::process::Output {
        Command::new(bin)
            .args(args)
            .env("KAIZEN_HOME", home)
            .output()
            .unwrap()
    }

    fn parse_pid(stdout: &[u8]) -> u32 {
        String::from_utf8_lossy(stdout)
            .lines()
            .find_map(|line| line.strip_prefix("pid: "))
            .unwrap()
            .parse()
            .unwrap()
    }

    fn session_id(pid: u32) -> u32 {
        let sid = unsafe { libc::getsid(pid as libc::pid_t) };
        assert!(
            sid > 0,
            "getsid failed: {}",
            std::io::Error::last_os_error()
        );
        sid as u32
    }

    fn stderr(output: &std::process::Output) -> String {
        String::from_utf8_lossy(&output.stderr).into_owned()
    }
}
