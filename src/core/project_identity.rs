// SPDX-License-Identifier: AGPL-3.0-or-later
//! Workspace label derived at export boundary.

use std::path::Path;

pub fn project_name(workspace: &Path) -> Option<String> {
    github_origin(workspace)
        .as_deref()
        .and_then(project_name_from_github_origin)
        .or_else(|| fallback_name(workspace))
}

pub fn project_name_from_github_origin(origin: &str) -> Option<String> {
    github_path(origin.trim()).and_then(repo_basename)
}

fn github_origin(workspace: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8(out.stdout).ok())?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn github_path(origin: &str) -> Option<&str> {
    origin
        .strip_prefix("https://github.com/")
        .or_else(|| origin.strip_prefix("http://github.com/"))
        .or_else(|| origin.strip_prefix("git@github.com:"))
        .or_else(|| origin.strip_prefix("ssh://git@github.com/"))
}

fn repo_basename(path: &str) -> Option<String> {
    path.rsplit('/')
        .next()
        .map(|s| s.strip_suffix(".git").unwrap_or(s))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn fallback_name(workspace: &Path) -> Option<String> {
    workspace
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_origin_forms() {
        let cases = [
            ("https://github.com/org/kaizen.git", "kaizen"),
            ("git@github.com:org/kaizen.git", "kaizen"),
            ("ssh://git@github.com/org/kaizen.git", "kaizen"),
        ];
        for (origin, expected) in cases {
            assert_eq!(
                project_name_from_github_origin(origin).as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn ignores_non_github_origin() {
        assert_eq!(
            project_name_from_github_origin("https://gitlab.com/org/kaizen.git"),
            None
        );
    }
}
