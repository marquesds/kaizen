//! Client-side redaction before enqueueing sync outbox rows.

use aho_corasick::AhoCorasick;
use regex::Regex;
use serde_json::Value;
use std::path::Path;
use std::sync::OnceLock;

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap())
}

fn win_drive_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?P<p>[a-z]):[/\\]").unwrap())
}

/// Patterns matched literally (secret-shaped substrings, common markers).
fn secret_needles() -> Vec<Vec<u8>> {
    vec![
        b"Bearer ".to_vec(),
        b"Authorization:".to_vec(),
        b"sk-".to_vec(),
        b"ghp_".to_vec(),
        b"gho_".to_vec(),
        b"xoxb-".to_vec(),
        b"AKIA".to_vec(), // AWS key prefix
    ]
}

/// Redact a full outbound event payload tree plus string leaves.
pub fn redact_payload(value: &mut Value, workspace: &Path, team_salt: &[u8; 32]) {
    redact_value(value, workspace, team_salt, true);
}

fn redact_value(v: &mut Value, workspace: &Path, team_salt: &[u8; 32], is_root: bool) {
    match v {
        Value::String(s) => {
            *s = redact_string(s, workspace, team_salt);
        }
        Value::Array(items) => {
            for x in items {
                redact_value(x, workspace, team_salt, false);
            }
        }
        Value::Object(map) => {
            map.retain(|k, _| !drop_key(k));
            let keys: Vec<String> = map.keys().cloned().collect();
            for k in keys {
                if let Some(val) = map.get_mut(&k) {
                    if k.ends_with("_TOKEN") || k.ends_with("_KEY") || k == "env" {
                        *val = Value::String("<REDACTED:secret>".to_string());
                    } else if k == "tool_args" || k == "command" {
                        redact_tool_args(val, workspace, team_salt);
                    } else {
                        redact_value(val, workspace, team_salt, false);
                    }
                }
            }
        }
        _ => {}
    }
    let _ = is_root;
}

fn drop_key(k: &str) -> bool {
    matches!(
        k,
        "user" | "git_email" | "prompt_text" | "completion_text" | "email"
    )
}

fn redact_tool_args(v: &mut Value, workspace: &Path, team_salt: &[u8; 32]) {
    match v {
        Value::Object(m) => {
            if let Some(Value::String(cmd)) = m.get_mut("command") {
                let redacted = redact_shell_command(cmd, workspace, team_salt);
                *cmd = redacted;
            }
            let keys: Vec<String> = m.keys().cloned().collect();
            for k in keys {
                if k != "command"
                    && let Some(val) = m.get_mut(&k)
                {
                    redact_value(val, workspace, team_salt, false);
                }
            }
        }
        _ => redact_value(v, workspace, team_salt, false),
    }
}

fn redact_shell_command(cmd: &str, workspace: &Path, team_salt: &[u8; 32]) -> String {
    let mut parts = cmd.split_whitespace();
    let Some(first) = parts.next() else {
        return String::new();
    };
    let rest: Vec<&str> = parts.collect();
    if rest.is_empty() {
        return redact_string(first, workspace, team_salt);
    }
    let redacted_rest: Vec<String> = rest
        .iter()
        .map(|t| {
            if looks_secret_token(t) {
                "<REDACTED:arg>".to_string()
            } else {
                redact_string(t, workspace, team_salt)
            }
        })
        .collect();
    format!(
        "{} {}",
        redact_string(first, workspace, team_salt),
        redacted_rest.join(" ")
    )
}

fn looks_secret_token(s: &str) -> bool {
    s.contains('=') && (s.contains("TOKEN") || s.contains("KEY") || s.contains("SECRET"))
        || s.starts_with("sk-")
        || s.starts_with("ghp_")
        || s.len() > 40
            && s.chars()
                .all(|c| c.is_alphanumeric() || "+/=_-".contains(c))
}

pub fn redact_string(s: &str, workspace: &Path, team_salt: &[u8; 32]) -> String {
    let mut out = s.to_string();
    out = email_re()
        .replace_all(&out, "<REDACTED:email>")
        .into_owned();
    out = replace_path_prefixes(&out, workspace, team_salt);
    scrub_secrets(&mut out);
    out
}

fn replace_path_prefixes(s: &str, workspace: &Path, team_salt: &[u8; 32]) -> String {
    let mut out = s.to_string();
    loop {
        let mut replaced = false;
        for prefix in ["/Users/", "/home/", "/var/folders/", "/private/var/"] {
            if let Some(idx) = out.find(prefix) {
                let tail = &out[idx + prefix.len()..];
                let end = tail
                    .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ')')
                    .unwrap_or(tail.len());
                let segment = &tail[..end];
                let placeholder = file_placeholder(workspace, team_salt, segment);
                out.replace_range(idx..idx + prefix.len() + end, &placeholder);
                replaced = true;
                break;
            }
        }
        if !replaced {
            break;
        }
    }
    out = win_drive_re()
        .replace_all(&out, |caps: &regex::Captures| {
            format!("<REDACTED:drive>{}", &caps["p"])
        })
        .into_owned();
    out
}

fn file_placeholder(workspace: &Path, team_salt: &[u8; 32], abs_tail: &str) -> String {
    let basename = abs_tail
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("file");
    let class = basename_class(basename);
    let rel_hash = rel_path_hash(workspace, team_salt, abs_tail);
    format!("<{rel_hash}:{class}>")
}

fn basename_class(name: &str) -> &'static str {
    if name.contains('.') { "file" } else { "path" }
}

fn rel_path_hash(workspace: &Path, team_salt: &[u8; 32], tail_after_prefix: &str) -> String {
    let synthetic =
        workspace.to_string_lossy().into_owned() + "/" + tail_after_prefix.trim_start_matches('/');
    let full = crate::sync::outbound::hash_with_salt(team_salt, synthetic.as_bytes());
    full.strip_prefix("blake3:")
        .map(|h| h[..8.min(h.len())].to_string())
        .unwrap_or_else(|| "hash".to_string())
}

fn scrub_secrets(s: &mut String) {
    let ac = AhoCorasick::new(secret_needles()).expect("patterns");
    let mut cursor = 0usize;
    while cursor < s.len() {
        let window = &s.as_bytes()[cursor..];
        if let Some(m) = ac.find(window) {
            let start = cursor + m.start();
            let mut end = start + m.len();
            while end < s.len() && !s.as_bytes()[end].is_ascii_whitespace() {
                end += 1;
            }
            s.replace_range(start..end, "<REDACTED:token>");
            cursor = start + "<REDACTED:token>".len();
        } else {
            break;
        }
    }
}

/// Returns true if `s` still contains forbidden path markers (for tests / guards).
fn forbidden_drive_users_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)[a-z]:\\Users\\").unwrap())
}

pub fn contains_forbidden_path_markers(s: &str) -> bool {
    s.contains("/Users/")
        || s.contains("/home/")
        || s.contains("/var/folders/")
        || s.contains("\\Users\\")
        || forbidden_drive_users_re().is_match(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_email() {
        let salt = [1u8; 32];
        let ws = Path::new("/proj");
        let r = redact_string("contact me at user@example.com ok", ws, &salt);
        assert!(!r.contains('@'));
        assert!(r.contains("REDACTED"));
    }

    #[test]
    fn redacts_users_path() {
        let salt = [2u8; 32];
        let ws = Path::new("/proj");
        let r = redact_string("file /Users/alice/secret.txt", ws, &salt);
        assert!(!r.contains("/Users/"));
    }

    #[test]
    fn drops_prompt_from_object() {
        let salt = [3u8; 32];
        let ws = Path::new("/w");
        let mut v = json!({"prompt_text": "x", "ok": true});
        redact_payload(&mut v, ws, &salt);
        assert!(!v.as_object().unwrap().contains_key("prompt_text"));
    }
}
