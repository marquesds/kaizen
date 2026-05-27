# ADR 009: Optional LLM Guidance Proposals

## Status
Accepted

## Context
Kaizen guidance now scores skills and Cursor rules from stored session outcomes.
The scorecard is deterministic, but users may want suggested edits for a
specific artifact. SkillOpt-style optimizers use an LLM for that proposal step.

## Decision
`kaizen guidance` stays deterministic by default. LLM proposal generation is
available only when the user passes `--llm` and `[guidance.proposals]` is
enabled. The retro engine remains covered by ADR 003: no LLM calls in retro
ranking.

## Consequences
- Default guidance scoring is reproducible, local, and offline-friendly.
- LLM proposal calls are explicit, opt-in, and use redacted scorecard evidence
  plus prior rejected-candidate memory.
- Direct file mutation requires `--apply`; candidates are stored and backed up.
- Candidate validation uses prompt-fingerprint experiments rather than treating
  before/after association as causality.
