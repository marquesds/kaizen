# Langfuse-Inspired Improvements: Roadmap

Five features drawn from [Langfuse](https://langfuse.com) that fill gaps in Kaizen's current
observability coverage.

Kaizen already covers: cost tracking, session collection, token counts, A/B experiments,
heuristic retros, telemetry export, LLM proxy. The gaps are qualitative — not quantitative.

## Features

| # | Feature | Doc | Effort |
|---|---------|-----|--------|
| F1 | LLM-as-a-Judge evaluations | [f1-llm-as-judge.md](f1-llm-as-judge.md) | 3d |
| F2 | Prompt/system-prompt version tracking | [f2-prompt-version-tracking.md](f2-prompt-version-tracking.md) | 2d |
| F3 | Session-level human feedback | [f3-human-feedback.md](f3-human-feedback.md) | 1.5d |
| F4 | Trace hierarchy (nested spans) | [f4-trace-hierarchy.md](f4-trace-hierarchy.md) | 3d |
| F5 | Dataset curation from production traces | [f5-dataset-curation.md](f5-dataset-curation.md) | 2d |

**Total:** ~11.5 dev days.

## Recommended sequence

| Sprint | Features | Rationale |
|--------|----------|-----------|
| 1 | F3 + F2 | Independent, low-risk, high signal. No external deps. |
| 2 | F5 | Builds on F3 (feedback → pin-to-dataset). |
| 3 | F1 + F4 | F1 needs external LLM calls; F4 is orthogonal and can parallelize. |

## How these features connect

F3 (feedback) provides human signal that feeds:

- F5 (datasets): bad/interesting sessions get pinned automatically via `--pin-to-dataset`.
- F1 (LLM judge): human scores become ground truth for calibrating rubrics.

F2 (prompt tracking) feeds:

- F1: judge prompts are versioned like production prompts.
- Experiments: new `Metric::CostByPrompt` variant compares metrics across fingerprints.

F4 (span trees) is standalone infrastructure that enriches all retro heuristics with subtree cost.

## Langfuse gap analysis

| Langfuse capability | Kaizen gap | Feature |
|--------------------|------------|---------|
| LLM-as-a-Judge evaluations | H1–H14 are count/threshold only | F1 |
| Prompt management with versioning | Sessions don't track which prompt was active | F2 |
| Human annotation / feedback loops | No way to label a session good/bad | F3 |
| Hierarchical trace trees | `tool_spans` is a flat list | F4 |
| Dataset curation from traces | Experiments are git-bound only | F5 |

## New heuristics summary

Each feature introduces at least one new retro heuristic:

| Heuristic | Feature | Trigger |
|-----------|---------|---------|
| H15 | F1 — LLM judge | ≥3 sessions with score < 0.4, or mean dropped >15% |
| H16 | F2 — Prompt tracking | ≥2 fingerprints with ≥5 sessions each; cost/error diverges >20% |
| H17 | F3 — Feedback | ≥2 sessions labeled `bad`/`regression`, or mean score ≤2.5 |
| H18 | F4 — Span trees | Max subtree depth ≥4, or max fan-out ≥8 |

F5 does not introduce a standalone heuristic; it extends the experiment framework instead.
