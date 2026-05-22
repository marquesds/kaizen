# AI Agent Rules

## 0. Communication Style (Always On)

Caveman mode **full** is default. Active every response, every session.
Drop: articles, filler (just/really/basically/actually/simply), pleasantries (sure/certainly/of course/happy to), hedging.
Fragments OK. Short synonyms. Technical terms exact. Code blocks unchanged.
Off only: user says "stop caveman" or "normal mode".
See `.cursor/skills/caveman/SKILL.md` for intensity levels and examples.

**Writing rule** — caveman default for: `.cursor/skills/`, `.cursor/rules/`, `AGENTS.md` (this file except where reader-facing), internal `.md`/`.mdc` not listed below, doc comments (`///`/`//!`) when contract-style, inline comments, plans, specs, ADRs, task breakdowns.

**Reader-facing exception** — full, clear technical prose (complete sentences, scannable structure): root [`README.md`](README.md), [`docs/**/*.md`](docs/README.md) user documentation, [`CONTRIBUTING.md`](CONTRIBUTING.md), and user-facing release notes in [`CHANGELOG.md`](CHANGELOG.md). Same tone as a strong engineering blog, not marketing fluff.

See `.cursor/rules/caveman-writing.mdc` for examples and the exception list.

## 1. Plan Mode Default

Enter plan mode for ANY task with 3+ steps or architectural decisions.
Goes sideways → STOP, re-plan immediately.

## 2. Subagent Strategy

Spawn subagent when ANY:
- Explore 3+ files for answer  → `explore`         e.g. "where is X handled across codebase"
- 5+ shell commands one task   → `shell`           e.g. release prep, log triage
- Task likely >15 turns        → `generalPurpose`  e.g. multi-file refactor with verification
- Browser/MCP test >2 actions  → `browser-use`     e.g. smoke test, screenshot diff
- Best-of-N experiment         → `best-of-n-runner` e.g. compare 3 implementations in worktrees

50k-token explore → 2k summary. 25× compression. One task per subagent.

## 3. Verification Before Done

Never mark complete without running `cargo test && cargo clippy -- -D warnings && cargo fmt --check`.
Ask: "Would staff engineer approve this?"

## 4. Demand Elegance (Balanced)

Nontrivial changes: pause, ask "Is there more elegant way?"
Hacky fix: "Knowing everything now, implement elegant solution."
Skip for simple obvious fixes.

## 5. Autonomous Bug Fixing

Bug report → fix it. No hand-holding.
Run `cargo test`. Zero context switching.

## 6. Documentation

Document preconditions, invariants, postconditions on public APIs.
No narrating obvious code.
`docs/` must stay current with every structural change.

## 7. Code Smell

- Max 200 lines per file (including `.md` files)
- Max 10 lines per function
- No cyclomatic complexity — prefer iterator chains + `match`

## 8. Design Principles

- **Functional Core / Imperative Shell** — pure `fn` handle logic; side effects at boundary only.
- **Sinks, Not Pipes** — components receive input, do work, stop. No cascading side effects.
  Ref: https://ianbull.com/posts/software-architecture/
- **AI-Ready Architecture** — module boundaries discoverable without reading internals.
  Follow SOLID. Deep modules with honest interfaces.
- **Simplicity First** — minimal code impact per change; no temporary workarounds.

## Documentation

Docs live in `docs/`. See [docs/README.md](docs/README.md) for index.

## 9. Shell Hygiene

Banned patterns — use tool equivalents instead:

| Banned | Use instead |
|---|---|
| `cat <file>` (>200 lines) | Read tool with offset/limit |
| `rg <pattern>` without `-m`/`--max-count`/`\| head` | Grep tool with head_limit |
| `find` without `-maxdepth` or result limit | Glob tool |
| `head`/`tail -n N` where N > 500 | Read tool with offset/limit |
| Read whole file >150 lines (no offset/limit) | Read with offset+limit OR Grep for signatures first |

## Hooks

Active hooks in `.cursor/hooks.json`. Read/shell caps enforce hygiene.
Claude Code: `.claude/settings.json` wires `cap-bash.sh` only.

## MCP

`cursor-ide-browser` MCP disabled for this workspace (saves ~2K tokens/turn from injected INSTRUCTIONS.md).
Browser testing → use `browser-use` subagent (has own MCP context). Re-enable via Cursor Settings → MCP & Integrations if needed.

## Session Context

`@`-attached files persist for entire session — remove from chat when no longer needed. Each costs tokens every turn.

**Cost telemetry**: Cursor sessions emit no token/usage data. For cost-sensitive or long-running work, use Claude Code so agentlens can measure spend.

## Agent Parity

`AGENTS.md` is single source of truth. `CLAUDE.md` points to it.
All agents (Cursor, Claude Code, Codex) read this file.

## Web Tool Parity

Every MCP tool must have web feature parity. Source of truth: `tests/mcp_tool_names.inc`.
When adding, removing, or renaming MCP tool:
1. Update web tool registry.
2. Add real web UI/API feature.
3. Do not add terminal-call shim.
4. Update parity test.

**Extended rules** in `.cursor/rules/*.mdc` — Cursor auto-loads; other agents read when relevant.

When adding/changing rule/skill:
1. Update `AGENTS.md` (or reference here)
2. Never edit `CLAUDE.md` — it points to `AGENTS.md`
3. Cursor-specific rules in `.cursor/rules/`; universal rules here

## Skills

Workflow guides in `.cursor/skills/<name>/SKILL.md`. Cursor injects `description:` frontmatter automatically — read SKILL.md when trigger fires. Don't load all at once.
