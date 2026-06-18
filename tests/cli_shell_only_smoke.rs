// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shell-only happy paths via `CARGO_BIN_EXE_kaizen` (MCP bypass). Uses an
//! isolated home so config and registry helpers do not touch real profile state.

use kaizen::shell::init::{KAIZEN_CLAUDE_HOOK_CMD, KAIZEN_CURSOR_HOOK_CMD};
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
    Ok(())
}

fn spawn(bin: &str, home: &Path, cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin)
        .current_dir(cwd)
        .env("HOME", home)
        .env("KAIZEN_HOME", home.join(".kaizen"))
        .arg("--no-daemon")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn: {e}"))
}

fn assert_init_global_paths(home: &Path, ws: &Path) -> anyhow::Result<()> {
    let hooks = home.join(".cursor/hooks.json");
    anyhow::ensure!(hooks.exists(), "~/.cursor/hooks.json not written");
    anyhow::ensure!(
        std::fs::read_to_string(&hooks)?.contains(KAIZEN_CURSOR_HOOK_CMD),
        "~/.cursor/hooks.json missing cursor hook cmd"
    );
    let settings = home.join(".claude/settings.json");
    anyhow::ensure!(settings.exists(), "~/.claude/settings.json not written");
    anyhow::ensure!(
        std::fs::read_to_string(&settings)?.contains(KAIZEN_CLAUDE_HOOK_CMD),
        "~/.claude/settings.json missing claude hook cmd"
    );
    anyhow::ensure!(
        home.join(".cursor/skills/kaizen-retro/SKILL.md").exists(),
        "kaizen-retro SKILL.md not written"
    );
    anyhow::ensure!(
        home.join(".cursor/skills/kaizen-eval/SKILL.md").exists(),
        "kaizen-eval SKILL.md not written"
    );
    anyhow::ensure!(
        !ws.join(".cursor/hooks.json").exists(),
        "workspace .cursor/hooks.json must not exist"
    );
    anyhow::ensure!(
        !ws.join(".claude/settings.json").exists(),
        "workspace .claude/settings.json must not exist"
    );
    Ok(())
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
    let init = spawn(b, &home, &ws, &["init"]);
    anyhow::ensure!(init.status.success(), "init failed");
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
        &[
            "telemetry",
            "configure",
            "--type",
            "file",
            "--path",
            "telemetry.ndjson",
        ],
        &["telemetry", "tail", "--json", "--no-follow"],
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
    assert_init_global_paths(&home, &ws)?;

    let cfg = std::fs::read_to_string(home.join(".kaizen/config.toml"))?;
    assert!(cfg.contains("type = \"file\""));
    assert!(cfg.contains("path = \"telemetry.ndjson\""));

    let plain = tmp.path().join("plain");
    std::fs::create_dir_all(&plain)?;
    std::fs::write(plain.join("main.rs"), b"fn main() {}\n")?;
    let init = spawn(b, &home, &plain, &["init"]);
    anyhow::ensure!(init.status.success(), "plain init failed");
    let out = spawn(b, &home, &plain, &["metrics", "index"]);
    assert!(
        out.status.success(),
        "non-git metrics index stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}
