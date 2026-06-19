use super::IngestSource;
use serde_json::Value;

pub(super) struct HookIdentity {
    pub(super) agent: String,
    pub(super) agent_update: Option<String>,
    pub(super) model: Option<String>,
    pub(super) trace_path: Option<String>,
}

pub(super) fn from_payload(source: IngestSource, payload: &Value) -> HookIdentity {
    let model = crate::collect::model_from_json::from_value(payload);
    let trace_path = text(payload, "transcript_path");
    let agent_update = inferred_agent(source, payload, model.as_deref(), trace_path.as_deref());
    let agent = agent_update
        .clone()
        .unwrap_or_else(|| source.agent().into());
    HookIdentity {
        agent,
        agent_update,
        model,
        trace_path,
    }
}

fn inferred_agent(
    source: IngestSource,
    payload: &Value,
    model: Option<&str>,
    path: Option<&str>,
) -> Option<String> {
    match source {
        IngestSource::Claude if codex_evidence(payload, model, path) => Some("codex".into()),
        IngestSource::Claude => None,
        _ => Some(source.agent().into()),
    }
}

fn codex_evidence(payload: &Value, model: Option<&str>, path: Option<&str>) -> bool {
    payload.get("turn_id").is_some()
        || path.is_some_and(codex_path)
        || model.is_some_and(codex_model)
}

fn codex_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.contains("/.codex/") || path.contains("\\.codex\\")
}

fn codex_model(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    ["gpt-", "o1", "o3", "o4", "codex", "kindle-", "nova-"]
        .iter()
        .any(|prefix| model.starts_with(prefix) || model.contains(prefix))
}

fn text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key)?.as_str().map(ToOwned::to_owned)
}
