# Kaizen local walkthrough

Self-recording script for people new to Kaizen. Aim for reveal, not flag tour:
agents leave evidence; Kaizen turns it into measurable improvement bets.

Pair with [kaizen-local-smoke-tests.md](kaizen-local-smoke-tests.md) for
repeatable setup and [tutorial](../tutorial/README.md) for deeper learning.

## Promise

> Kaizen is a local-first feedback loop for coding agents: capture -> summarize
> -> propose a change -> measure whether it worked.

Keep local-first promise explicit: default data lives in `.kaizen/kaizen.db`.
Nothing leaves disk unless sync/provider queries are configured.

## Prep

Best demo: use a real repo where agents already ran. `retro` feels magical when
it names a hot file, repeated tool loop, unused skill, or model-cost pattern.
For asciinema, keep font large, terminal width near 100 columns. The automated driver defaults target **readable onboarding** (roughly **3–5+ minutes** of playback depending on machine speed and store size): short gaps via **`KAIZEN_DEMO_PAUSE_SEC`** (default **`1.2`**) and longer dwell after dense output via **`KAIZEN_DEMO_READ_PAUSE_SEC`** (default **`4.8`**—for sessions table, summary, summary JSON preview, metrics, retro, exp report). For faster regeneration or CI smoke, tighten both, e.g. **`KAIZEN_DEMO_PAUSE_SEC=0.6 KAIZEN_DEMO_READ_PAUSE_SEC=1.5`**.

**Automated WOW demo** uses **real** Cursor/Claude sessions from transcript scan in the repo (no synthetic hook seed). From repo root after `cargo build`:

```bash
KAIZEN_BIN="$PWD/target/debug/kaizen" asciinema rec --overwrite \
  --window-size 100x30 \
  -c "zsh -f $PWD/scripts/record-kaizen-walkthrough.sh" \
  docs/plans/demo-wow.cast
asciinema play docs/plans/demo-wow.cast
```

**Prerequisites:** Populate the local store first (e.g. **`kaizen sessions list --refresh`** in a TTY once); the recording driver avoids **`--refresh`** so the cast stays fast. After changing Kaizen versions, **`--refresh`** again if **summary** still shows **$0.00** while transcripts include token usage (ingest must rescan to fill **`cost_usd_e6`** estimates). At least one session must show in **`kaizen sessions list`**. Override workspace with **`KAIZEN_DEMO_ROOT=/path/to/repo`**. Use **`KAIZEN_DEMO_ISOLATED=1`** for a disposable **`KAIZEN_HOME`** only (machine registry); **`.kaizen/` in the repo is still written** (including `exp` steps). **`KAIZEN_DEMO_ALLOW_EMPTY=1`** skips the experiment block when count is zero (debug only). **`KAIZEN_DEMO_REINDEX=1`** runs **`kaizen metrics index --force`** and a second **`kaizen metrics`** (adds tens of seconds—omit for short clips). The driver uses **`kaizen sessions list --json --limit 1`** internally so JSON stays tiny while tagging the newest session.

**Experiment reports** are cache-first like `summary`: **`kaizen exp report`** does not force a full transcript rescan unless you pass **`--refresh`**. Use **`--refresh`** when the store may be stale.

Use **`zsh -f`** so a custom `~/.zshrc` cannot inject aliases (for example shadowing the driver’s **`pause_short`** / **`pause_read`** helpers). Driver: [`scripts/record-kaizen-walkthrough.sh`](../../scripts/record-kaizen-walkthrough.sh).
`docs/plans/*.cast` is gitignored (paths/timestamps vary by machine).

For a repeatable demo, isolate machine state:

```bash
TMP="$(mktemp -d)"
export KAIZEN_HOME="$TMP/kaizen-home"
mkdir -p "$KAIZEN_HOME"
export WORKDIR="/path/to/demo-repo"
cd "$WORKDIR"
```

If no real sessions exist, seed one event in a disposable workspace:

```bash
export WORKDIR="$TMP/ws-main"
mkdir -p "$WORKDIR"
cd "$WORKDIR"

kaizen init
printf '%s\n' \
  '{"event":"SessionStart","session_id":"demo-s1","timestamp_ms":1714000000000}' \
  | kaizen ingest hook --source cursor --workspace "$WORKDIR"
```

Host note: prefer isolated `KAIZEN_HOME`. If daemon details distract, add
`--no-daemon` to read commands or set `KAIZEN_DAEMON=0`.

If an AI drives the terminal, give it one job: run commands, pause after
interesting output, avoid remote sync and destructive cleanup.

```text
You are driving a recorded Kaizen demo. Run only commands from this walkthrough.
Pause after each reveal. If output is thin, say the repo needs more sessions.
Do not configure sync, telemetry push, provider queries, eval, or cleanup.
```

## Beat 1: setup, 0 to 5 min

Start with what Kaizen is not: not an agent replacement, hosted account, or
self-grading model loop for core retro.

```bash
kaizen init
kaizen doctor
```

Say what appeared: `.kaizen/config.toml` is workspace config, `.kaizen/kaizen.db`
is local SQLite storage after ingest, Cursor and Claude Code hooks call
`kaizen ingest hook`, and other agents can arrive through transcript tails.

Recording cue: "So it watches the agent work I already do?" Yes. Kaizen observes
existing work, then gives it back as evidence.

## Beat 2: first reveal, 5 to 10 min

```bash
kaizen sessions list
kaizen summary
```

Narrate: sessions are units of agent work; summary rolls up agents, models,
cost, and tool volume; cache-first reads keep the common path fast.

If someone asks about automation:

```bash
kaizen summary --json
```

Keep JSON short. Message: humans read the report; agents and dashboards can
consume the same local truth.

Recording cue: "This beats transcript archaeology."

## Beat 3: repo intelligence, 10 to 15 min

```bash
kaizen metrics --days 7
```

After big refactors, rebuild repo facts first:

```bash
kaizen metrics index --force
kaizen metrics --days 7
```

The scripted **`record-kaizen-walkthrough.sh`** skips **`metrics index --force`** by default so playback stays ~60s; set **`KAIZEN_DEMO_REINDEX=1`** when you want that beat in the recording.

Narrate: `summary` asks "how much agent work happened?" `metrics` asks "where
did that work hit this repo?" Hot files, slow tools, and token-heavy paths
connect agent behavior to code structure.

First wow: Kaizen is not only counting spend. It connects agent work to files,
graph facts, and repeated repo friction.

Recording cue: "It knows which part of this repo makes agents expensive?" Yes,
when local events and repo index have enough signal.

## Beat 4: improvement bet, 15 to 22 min

```bash
kaizen retro --dry-run --days 7
```

Read it like a staff engineer: start with the high-confidence bet, point to
evidence, savings, and effort, then map action to repo change: add a rule, split
a file, delete an unused skill, stabilize a failing command, or change model
routing.

Second wow: Kaizen is not a passive dashboard. It proposes small, testable
changes from observed agent friction.

If output is thin, say: "This repo needs more sessions before retro gets
opinionated." Then show the sample in [README.md](../../README.md#demo).

## Beat 5: closed loop, 22 to 28 min

Turn one retro bet into an experiment:

```bash
kaizen exp power --metric tokens_per_session --baseline-n 50

kaizen exp new --name demo-rule \
  --hypothesis "repo rule cuts repeated shell failures" \
  --change "add a local smoke command and document env vars" \
  --metric tokens_per_session \
  --bind manual \
  --duration-days 14 --target-pct -10
```

Capture the printed experiment id:

```bash
kaizen exp list
kaizen exp status <id>
kaizen exp tag <id> --session <sid> --variant treatment
kaizen exp report <id>
```

Do not fake certainty. The report gets useful after enough control and
treatment sessions. Wow = a proposed repo change becomes measurable instead of
"seems better."

## Optional power move: live browser

Use this only with real sessions:

```bash
kaizen tui
```

Show contrast: `sessions show <id>` gives metadata, `sessions tree <id>` shows
nested tool spans, and `kaizen tui` browses turns, events, and tool detail. Keep
it short. TUI is texture, not the main story.

## Skip in first demo

- Remote provider queries with `--source provider` or `--source mixed`.
- Sync endpoints and live ingest servers.
- PostHog, Datadog, OTLP, or `telemetry push`.
- `kaizen eval run`, because it needs judge credentials.
- Full MCP wiring. Mention `kaizen mcp`, then point to
  [Part 8](../tutorial/08-mcp.md).

## Close

End with:

> The agent already did the work. Kaizen makes the work inspectable, gives you a
> concrete improvement bet, and gives you a way to measure whether the bet paid
> off.

Next command for the room:

```bash
cd /path/to/their-repo
kaizen init
kaizen doctor
```
