---
name: kaizen-retro
description: >
  Surface ranked bets to make agents cheaper, faster, and more accurate in
  this codebase. Use when the user asks "what should I improve", "run retro",
  "agent productivity bets", "audit my skills", or invokes /retro.
---

# Kaizen retro

1. From the **workspace root** (the repo the user cares about), run:

   ```bash
   kaizen retro --json --days 7
   ```

   If `kaizen` is not on `PATH`, use the full path to the binary.

2. Parse the JSON stdout as a single `Report` object (see `meta`, `top_bets`, `stats`).

3. Reply with **at most three** items from `top_bets`, each including:
   - **Title** and **heuristic id** (e.g. H2)
   - **Hypothesis** (`hypothesis`)
   - **Expected impact** (`expected_tokens_saved_per_week` as approximate tokens/week saved)
   - **Evidence** (bullet list from `evidence`)
   - **Apply step** (`apply_step`)

4. Pick **one** bet as the suggested next action and say why briefly.

5. Do **not** run destructive commands (`rm`, mass edits) unless the user explicitly asks. The CLI `apply_step` is for the human.

6. If `top_bets` is empty, say that the window had no strong signals and suggest widening the window (`kaizen retro --days 30 --json`) or collecting more sessions.
