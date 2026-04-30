---
name: kaizen-eval
description: >
  Evaluate agent session quality with LLM-as-a-Judge. Use when the user asks
  "how did the agent do", "score this session", "eval this session", or invokes
  /eval.
---

# Kaizen eval

1. To **preview** the judge prompt for a session (no LLM call):

   ```bash
   kaizen eval prompt <session-id>
   ```

   Pipe the output to any LLM or inspect it manually.

2. To **run automated scoring** (requires `[eval] enabled = true` in `.kaizen/config.toml`):

   ```bash
   kaizen eval run --since 7d
   ```

3. To **review stored scores**:

   ```bash
   kaizen eval list --min-score 0
   ```

4. Sessions with `flagged = true` (score < 0.4) signal tool-efficiency problems.
   Cross-reference with `kaizen retro --json` to see if H15 has fired.

5. Do **not** call the LLM endpoint unless `eval.enabled = true` or the user
   explicitly asks to run scoring.
