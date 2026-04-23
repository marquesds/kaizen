---
name: session-handoff
description: >
  Pause/resume long agent sessions. Use ONLY when user explicitly says
  "create handoff" or "resume handoff". Do not auto-trigger on context fill.
---

# Session Handoff

Save mid-flight session state. Next agent picks up cold with zero ambiguity.

## When To Use

| Trigger | Action |
|---|---|
| Context ~50% full AND task <90% complete | CREATE (proactive, early) |
| Context >75% full, work not done | CREATE (mandatory) |
| User says "pause", "save state", "create handoff" | CREATE |
| Major task milestone, want save point | CREATE |
| 5+ files edited, complex debug, big decisions | CREATE (proactive) |
| User mentions existing handoff / "resume" | RESUME |
| Continuing chained work | RESUME + chain |

### 50% Rule

Cheaper to hand off early than crash at 90%. At ~50% context:

1. Estimate task completion: <90% done?
2. Yes → CREATE handoff now, spawn fresh session for remainder
3. No (≥90%) → push through, finish in current session

Why: handoff write itself burns ~5-10% context. Doing it at 75% leaves
no room for resume-side validation. At 50% you still have headroom to
write quality handoff + commit + close cleanly.

Pairs with `session-budget` skill — see that for tool-call thresholds.

## Storage

```
.handoffs/YYYY-MM-DD-HHMMSS-<slug>.md
```

Committed to repo. Team continuity > local privacy. Strip secrets at create.

## CREATE Workflow

### Step 1: Gather State

Run in parallel:

```bash
git status --short
git log --oneline -10
git diff --stat
git branch --show-current
date "+%Y-%m-%d-%H%M%S"
```

### Step 2: Write Handoff

Copy `template.md` → `.handoffs/<timestamp>-<slug>.md`. Fill all `[TODO]`.

Required sections (must have content):
- **Current State** — what's happening right now, one paragraph
- **Critical Context** — what next agent MUST know to not break things
- **Next Steps** — ordered, actionable, first item runnable immediately
- **Decisions Made** — choice + rationale + what rejected
- **Modified Files** — list with one-line purpose
- **Verification Status** — `cargo test && cargo clippy` passing? failing?

### Step 3: Validate

Self-check before finalizing:

| Check | Pass criteria |
|---|---|
| No `[TODO]` placeholders | grep `\[TODO\]` returns nothing |
| No secrets | no API keys, tokens, passwords, .env contents |
| File refs exist | every `src/...` or `tests/...` path resolves |
| Next step actionable | first item is concrete command or edit |
| Verification noted | tests/clippy state declared |

Reject handoff if any check fails. Fix and re-validate.

### Step 4: Commit (Optional)

If user asks: commit `.handoffs/<file>.md` with message
`chore(handoff): <slug>`. Don't auto-commit otherwise.

## RESUME Workflow

### Step 1: Find Handoff

```bash
ls -lt .handoffs/ | head -10
```

User picks, or grab latest if obvious.

### Step 2: Check Staleness

```bash
HANDOFF=.handoffs/<file>.md
HANDOFF_DATE=$(stat -f %m "$HANDOFF")
git log --since=@$HANDOFF_DATE --oneline
git diff --stat $(git log --before=@$HANDOFF_DATE -1 --format=%H)..HEAD
```

| Signal | Level | Action |
|---|---|---|
| <10 commits, <24h old | FRESH | Resume directly |
| 10-30 commits or 1-7 days | STALE | Read diff, verify assumptions |
| >30 commits or >7 days | DEAD | Create new handoff, archive old |

### Step 3: Load Context

1. Read full handoff file
2. If `Continues from:` present → read predecessor too
3. Read every file listed in **Modified Files**
4. Verify branch matches `git branch --show-current`

### Step 4: Verify Assumptions Hold

Walk **Critical Context** section. For each assumption:
- Still true? → continue
- Changed? → note in new handoff, adjust plan
- Unknown? → grep/read to verify before acting

### Step 5: Execute Next Step

Start at **Next Steps** item #1. Run, verify, mark done in handoff or carry forward.

## Chaining

Long projects → chain handoffs:

```
.handoffs/2026-04-18-100000-collector-ingest.md
    ↓ Continues from above
.handoffs/2026-04-18-160000-collector-ingest-pt2.md
    ↓ Continues from above
.handoffs/2026-04-19-090000-collector-ingest-pt3.md
```

Each new handoff:
- `Continues from:` field at top points to prior file
- Marks superseded sections in prior handoff
- Carries forward unresolved items only

## Integration With Project

| System | Role |
|---|---|
| `.handoffs/` | Transient session state (snapshot for resume) |
| `docs/` | Permanent architecture + decisions |
| git | Source of truth for code state |

## Anti-Patterns

| Anti-Pattern | Fix |
|---|---|
| Handoff with `[TODO]` left | Validate before finalizing |
| Secrets pasted in "current state" | Redact before write |
| Vague next step ("continue work") | Concrete: file + line + action |
| Skip staleness check on resume | Always run git diff vs handoff date |
| Handoff every minor pause | Only create at real session boundary |

## Red Flags

- `.handoffs/` directory has 50+ files → archive old, raise threshold
- Same handoff resumed 5+ times → split work, ship intermediate result
- Handoff size >500 lines → context too big, split into sub-tasks first
- No `cargo test` status declared → unknown verification = unknown state
