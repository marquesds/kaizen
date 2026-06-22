use crate::core::event::{Event, EventKind};
use crate::visualization::TraceDetail;
use serde_json::{Value, json};

const MAX_COMMAND_CHARS: usize = 600;

pub(super) fn prepare(detail: &mut TraceDetail) {
    detail.prompt = prompt_from_events(&detail.events)
        .or_else(|| prompt_from_trace(&detail.session.trace_path));
    detail.events.iter_mut().for_each(compact_event);
}

fn compact_event(event: &mut Event) {
    event.payload = event_summary(event)
        .map(|summary| json!({"summary": summary}))
        .unwrap_or(Value::Null);
}

fn event_summary(event: &Event) -> Option<String> {
    (event.kind == EventKind::ToolCall)
        .then(|| payload_summary(&event.payload))
        .flatten()
        .map(|value| compact_text(&value, MAX_COMMAND_CHARS))
}

fn payload_summary(payload: &Value) -> Option<String> {
    pointer_text(
        payload,
        &["/tool_input/command", "/input/command", "/command", "/cmd"],
    )
    .or_else(|| argument_summary(payload))
    .or_else(|| pointer_text(payload, &["/tool_input/file_path", "/input/path", "/path"]))
}

fn argument_summary(payload: &Value) -> Option<String> {
    let raw = pointer_text(payload, &["/arguments", "/function/arguments"])?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;
    pointer_text(&parsed, &["/cmd", "/command", "/path", "/file_path"])
}

fn pointer_text(value: &Value, pointers: &[&str]) -> Option<String> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer)?.as_str().map(ToOwned::to_owned))
}

fn prompt_from_events(events: &[Event]) -> Option<String> {
    events
        .iter()
        .rev()
        .find_map(|event| crate::core::prompt_text::from_value(&event.payload))
}

fn prompt_from_trace(raw: &str) -> Option<String> {
    super::prompt_cache::from_trace(raw)
}

pub(super) fn prompt_from_line(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    value
        .get("payload")
        .and_then(crate::core::prompt_text::from_value)
        .or_else(|| crate::core::prompt_text::from_value(&value))
}

fn compact_text(value: &str, limit: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated = normalized.chars().take(limit).collect::<String>();
    if normalized.chars().count() > limit {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn trusted_trace_returns_latest_user_prompt() {
        let _guard = crate::core::paths::test_lock::global().lock().unwrap();
        let (temp, path, _home) = trace_fixture();
        let line = json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>hidden</environment_context>"},{"type":"input_text","text":"Show commands"}]}});
        let filler = format!(
            "{}\n",
            json!({"type":"event_msg","payload":{"type":"noise"}})
        )
        .repeat(10_000);
        std::fs::write(&path, format!("{line}\n{filler}")).unwrap();
        assert_eq!(
            prompt_from_trace(path.to_str().unwrap()).as_deref(),
            Some("Show commands")
        );
        drop(temp);
    }

    #[test]
    fn codex_arguments_expose_command_only() {
        let payload = json!({"arguments":"{\"cmd\":\"rg parser src\",\"max_output_tokens\":1000}"});
        assert_eq!(payload_summary(&payload).as_deref(), Some("rg parser src"));
    }

    #[test]
    fn objective_wrapper_returns_user_objective() {
        let wrapped =
            "<codex_internal_context><objective>Fix identity</objective></codex_internal_context>";
        assert_eq!(
            crate::core::prompt_text::from_value(&json!({"prompt": wrapped})).as_deref(),
            Some("Fix identity")
        );
    }

    #[test]
    fn goal_update_wrapper_returns_user_objective() {
        let wrapped = "<codex_internal_context><untrusted_objective>Show prompt</untrusted_objective>Budget: hidden</codex_internal_context>";
        assert_eq!(
            crate::core::prompt_text::from_value(&json!({"prompt": wrapped})).as_deref(),
            Some("Show prompt")
        );
    }

    fn trace_fixture() -> (tempfile::TempDir, PathBuf, HomeGuard) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(".codex/sessions/session.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let home = HomeGuard::set(temp.path().into());
        (temp, path, home)
    }

    struct HomeGuard(Option<OsString>);

    impl HomeGuard {
        fn set(value: OsString) -> Self {
            let old = std::env::var_os("HOME");
            unsafe { std::env::set_var("HOME", value) };
            Self(old)
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => unsafe { std::env::set_var("HOME", value) },
                None => unsafe { std::env::remove_var("HOME") },
            }
        }
    }
}
