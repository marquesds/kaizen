use std::path::Path;

pub(super) fn git_remote_origin(repo: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| trim_stdout(output.stdout))?
}

fn trim_stdout(stdout: Vec<u8>) -> Option<String> {
    String::from_utf8(stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
