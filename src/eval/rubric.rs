// SPDX-License-Identifier: AGPL-3.0-or-later

#[derive(Debug, Clone)]
pub struct Rubric {
    pub id: &'static str,
    pub name: &'static str,
    pub prompt_template: &'static str,
}

const TOOL_EFFICIENCY_V1: Rubric = Rubric {
    id: "tool-efficiency-v1",
    name: "Tool Efficiency",
    prompt_template: "You are an expert code-agent evaluator.\n\
        Score the following agent session on tool efficiency (0.0 = very poor, 1.0 = excellent).\n\
        Penalise redundant tool calls, excessive retries, and ignored errors.\n\
        Reward sessions that reach the goal with minimal steps.\n\n\
        Session summary:\n{summary}\n\n\
        Reply with JSON only: {\"score\": <float 0-1>, \"rationale\": \"<one sentence>\"}",
};

pub fn by_id(id: &str) -> Option<&'static Rubric> {
    match id {
        "tool-efficiency-v1" => Some(&TOOL_EFFICIENCY_V1),
        _ => None,
    }
}
