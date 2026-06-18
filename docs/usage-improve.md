# Experiments and Evaluation

[Back to CLI index](usage.md).

## `kaizen exp`

A/B experiments with bootstrap confidence intervals, sample-ratio mismatch
checks, and sequential testing.

```bash
kaizen exp power --metric tokens_per_session --baseline-n 50
kaizen exp power --metric tokens_per_session --baseline-n 50 --refresh

kaizen exp new --name add-skill \
  --hypothesis "skill cuts tokens" \
  --change "add .cursor/skills/x" \
  --metric tokens_per_session \
  --bind git --duration-days 14 --target-pct -10

kaizen exp start <id>
kaizen exp list
kaizen exp status <id>
kaizen exp tag <id> --session <sid> --variant treatment
kaizen exp report <id>
kaizen exp report <id> --json
kaizen exp report <id> --refresh
kaizen exp conclude <id>
kaizen exp archive <id>
```

Bindings can use git, branches, prompts, or manual tags. Metrics include tokens,
cost, success rate, tool loops, duration, files, and prompt-specific outcomes.
See [experiments.md](experiments.md).

## `kaizen cases`, `rules`, `alerts`, and `review`

These commands form a local trace-to-case workflow:

```bash
kaizen cases mine --since 14d
kaizen cases create --session <id> --reason "bad tool loop" --label regression
kaizen cases list --json
kaizen cases show <case-id>
kaizen cases archive <case-id>

kaizen rules create --name shell-loops --filter 'tool:bash' \
  --action queue_review --message "review shell-heavy session"
kaizen rules run --since 7d
kaizen rules disable <rule-id>
kaizen rules enable <rule-id>

kaizen alerts check --days 7 --json
kaizen review list
kaizen review show <review-id>
kaizen review resolve <review-id>
kaizen review dismiss <review-id>
```

Built-in alerts cover cost spikes, eval regression, bad feedback, error-rate
spikes, context pressure, retry cascades, and max-token truncation.

## `kaizen eval`

LLM-as-a-Judge evaluation requires `[eval].enabled = true` and either
`[eval].api_key` or `ANTHROPIC_API_KEY`.

```bash
kaizen eval run
kaizen eval run --since-days 14
kaizen eval run --dry-run
kaizen eval run --dry-run --json
kaizen eval list
kaizen eval list --min-score 0
kaizen eval list --json
kaizen eval prompt <session-id>
kaizen eval prompt <session-id> --rubric tool-efficiency-v1
```

The default judge uses the `tool-efficiency-v1` rubric. Results are stored in
SQLite and low scores feed retro heuristic H15. `eval prompt` renders the judge
prompt without calling a model.

## `kaizen prompt`

Prompt tracking fingerprints `CLAUDE.md`, `AGENTS.md`, Cursor rules, and Cursor
skills at session start. A changed prompt at session end records a
`prompt_changed` event.

```bash
kaizen prompt list
kaizen prompt list --json
kaizen prompt show <fingerprint>
kaizen prompt show <fingerprint> --json
kaizen prompt diff <fp_a> <fp_b>
```

Retro heuristic H16 compares prompt versions when both have enough sessions.

## `kaizen sessions annotate` and `kaizen feedback`

Attach a score, label, and optional note to a session:

```bash
kaizen sessions annotate <id> --score 2 --label bad \
  --note "hallucinated file path"
kaizen sessions annotate <id> --label interesting

kaizen feedback list
kaizen feedback list --label bad
kaizen feedback list --since 7d
kaizen feedback list --since 7d --json
```

Labels are `good`, `bad`, `interesting`, `bug`, and `regression`. Scores range
from 1 to 5. Feedback appears in `sessions show` and feeds retro heuristic H17.
