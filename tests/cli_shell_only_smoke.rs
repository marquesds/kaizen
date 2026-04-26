// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shell-only happy paths via `CARGO_BIN_EXE_kaizen` (MCP bypass). Uses an
//! isolated `HOME` so telemetry config helpers do not touch the real profile.

use std::path::Path;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kaizen")
}

fn run_git(ws: &Path, args: &[&str]) -> anyhow::Result<()> {
    let s = Command::new("git").arg("-C").arg(ws).args(args).status()?;
    anyhow::ensure!(s.success(), "git {:?}", args);
    Ok(())
}

fn prepare_ws(tmp: &Path) -> anyhow::Result<()> {
    run_git(tmp, &["init"])?;
    std::fs::write(tmp.join("README.md"), b"x\n")?;
    run_git(tmp, &["add", "README.md"])?;
    run_git(
        tmp,
        &[
            "-c",
            "user.email=x@x",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "c",
        ],
    )?;
    kaizen::shell::init::init_text(Some(tmp))?;
    Ok(())
}

fn spawn(bin: &str, home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin)
        .current_dir(cwd)
        .env("HOME", home)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn: {e}"))
}

#[test]
fn shell_only_smoke_matrix() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home)?;
    let ws = tmp.path().join("repo");
    std::fs::create_dir_all(&ws)?;
    prepare_ws(&ws)?;
    let b = bin();
    let cases: &[&[&str]] = &[
        &["doctor"],
        &["guidance", "--json"],
        &["gc", "--days", "365"],
        &["completions", "bash"],
        &["telemetry", "print-schema"],
        &["telemetry", "doctor"],
        &["telemetry", "print-effective-config"],
        &["eval", "list"],
        &["prompt", "list"],
        &["feedback", "list"],
        &["sync", "status"],
        &["exp", "list"],
        &["sessions", "list"],
        &["summary", "--json"],
        &["insights"],
        &["metrics"],
        &["retro", "--dry-run"],
        &["init"],
        &["sync", "run", "--once"],
        &["metrics", "index"],
        &["telemetry", "init"],
        &["telemetry", "configure"],
    ];
    for a in cases {
        let args: &[&str] = a;
        let out = spawn(b, &home, &ws, args);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "args={args:?} code={:?} stderr={err}",
            out.status.code()
        );
        if args == ["completions", "bash"] {
            assert!(!out.stdout.is_empty(), "completions bash empty");
        }
        if args == ["telemetry", "print-schema"] {
            assert!(!out.stdout.is_empty(), "print-schema empty");
        }
    }
    Ok(())
}
