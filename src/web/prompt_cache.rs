use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_SCAN_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 128;

#[derive(Clone)]
struct Entry {
    len: u64,
    prompt: Option<String>,
}

pub(super) fn from_trace(raw: &str) -> Option<String> {
    let path = trusted_trace(raw)?;
    let len = std::fs::metadata(&path).ok()?.len();
    let mut cache = prompt_cache().lock().ok()?;
    if let Some(entry) = cache.get_mut(&path) {
        return refresh(&path, len, entry);
    }
    let prompt = read_latest(&path, len.saturating_sub(MAX_SCAN_BYTES));
    insert(&mut cache, path, len, prompt.clone());
    prompt
}

fn refresh(path: &Path, len: u64, entry: &mut Entry) -> Option<String> {
    if len != entry.len {
        let start = if len < entry.len {
            len.saturating_sub(MAX_SCAN_BYTES)
        } else {
            entry.len.max(len.saturating_sub(MAX_SCAN_BYTES))
        };
        entry.prompt = read_latest(path, start).or_else(|| entry.prompt.clone());
        entry.len = len;
    }
    entry.prompt.clone()
}

fn insert(cache: &mut HashMap<PathBuf, Entry>, path: PathBuf, len: u64, prompt: Option<String>) {
    if cache.len() >= MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(path, Entry { len, prompt });
}

fn read_latest(path: &Path, start: u64) -> Option<String> {
    let text = read_from(path, start)?;
    text.lines()
        .rev()
        .find_map(super::event_display::prompt_from_line)
}

fn read_from(path: &Path, start: u64) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn trusted_trace(raw: &str) -> Option<PathBuf> {
    let path = std::fs::canonicalize(raw).ok()?;
    let home = std::fs::canonicalize(std::env::var_os("HOME")?).ok()?;
    trusted_root(&path, &home).then_some(path)
}

fn trusted_root(path: &Path, home: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
        && [".codex/sessions", ".claude/projects", ".cursor/projects"]
            .iter()
            .any(|root| path.starts_with(home.join(root)))
}

fn prompt_cache() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_replaces_prompt_after_trace_truncation() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let line = serde_json::json!({"role":"user","content":"New prompt"});
        std::fs::write(temp.path(), format!("{line}\n")).unwrap();
        let len = std::fs::metadata(temp.path()).unwrap().len();
        let mut entry = Entry {
            len: len + 100,
            prompt: Some("Old prompt".into()),
        };
        assert_eq!(
            refresh(temp.path(), len, &mut entry).as_deref(),
            Some("New prompt")
        );
    }
}
