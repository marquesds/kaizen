use serde_json::Value;

const MAX_PROMPT_CHARS: usize = 8_000;

pub(crate) fn from_value(value: &Value) -> Option<String> {
    direct(value)
        .or_else(|| user_message(value))
        .map(|text| compact(&text))
}

fn direct(value: &Value) -> Option<String> {
    ["/prompt", "/user_prompt"]
        .iter()
        .find_map(|path| value.pointer(path)?.as_str().and_then(clean))
}

fn user_message(value: &Value) -> Option<String> {
    let message = value.get("message").unwrap_or(value);
    (message.get("role")?.as_str()? == "user")
        .then(|| content(message.get("content")?))
        .flatten()
}

fn content(value: &Value) -> Option<String> {
    value.as_str().and_then(clean).or_else(|| {
        value
            .as_array()?
            .iter()
            .rev()
            .find_map(|part| part.get("text")?.as_str().and_then(clean))
    })
}

fn clean(raw: &str) -> Option<String> {
    let value = objective(raw).unwrap_or(raw).trim();
    (!value.is_empty() && !ignored(value)).then(|| value.to_string())
}

fn objective(raw: &str) -> Option<&str> {
    tagged(raw, "<objective>", "</objective>")
        .or_else(|| tagged(raw, "<untrusted_objective>", "</untrusted_objective>"))
}

fn tagged<'a>(raw: &'a str, open: &str, close: &str) -> Option<&'a str> {
    raw.split_once(open)?.1.split_once(close).map(|pair| pair.0)
}

fn ignored(value: &str) -> bool {
    value.starts_with("<environment_context>") || value.starts_with("# AGENTS.md instructions")
}

fn compact(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated = normalized
        .chars()
        .take(MAX_PROMPT_CHARS)
        .collect::<String>();
    if normalized.chars().count() > MAX_PROMPT_CHARS {
        format!("{truncated}...")
    } else {
        truncated
    }
}
