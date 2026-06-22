// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub(super) struct TestDaemon {
    tmp: tempfile::TempDir,
    home: PathBuf,
    workspace: PathBuf,
}

impl TestDaemon {
    pub(super) fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let workspace = create_workspace(tmp.path(), "repo");
        std::fs::create_dir_all(&home).unwrap();
        Self {
            tmp,
            home,
            workspace,
        }
    }

    pub(super) fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub(super) fn workspace_str(&self) -> &str {
        self.workspace.to_str().unwrap()
    }

    pub(super) fn home(&self) -> &Path {
        &self.home
    }

    pub(super) fn create_workspace(&self, name: &str) -> PathBuf {
        create_workspace(self.tmp.path(), name)
    }

    pub(super) fn db_path(&self, workspace: &Path) -> PathBuf {
        let slug = kaizen::core::paths::workspace_slug(workspace);
        self.home
            .join(".kaizen/projects")
            .join(slug)
            .join("kaizen.db")
    }

    pub(super) fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    pub(super) fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kaizen"));
        command.args(args).env("HOME", &self.home);
        command.env("KAIZEN_HOME", self.home.join(".kaizen"));
        command
    }

    pub(super) fn write_codex_session(&self, id: &str) {
        let dir = self.home.join(".codex/sessions/2026/06/22");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{id}.jsonl")),
            codex_rows(id, &self.workspace),
        )
        .unwrap();
    }

    fn stop(&self) {
        let _ = self.run(&["daemon", "stop"]);
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        self.stop();
    }
}

fn create_workspace(root: &Path, name: &str) -> PathBuf {
    let workspace = root.join(name);
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::canonicalize(workspace).unwrap()
}

fn codex_rows(id: &str, workspace: &Path) -> String {
    let ws = workspace.to_string_lossy();
    format!(
        "{{\"timestamp\":\"2026-06-22T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"cwd\":\"{ws}\"}}}}\n\
         {{\"timestamp\":\"2026-06-22T12:00:01Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"call_id\":\"call-1\",\"name\":\"exec_command\",\"arguments\":\"{{}}\"}}}}\n"
    )
}

pub(super) fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        text(output),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}
