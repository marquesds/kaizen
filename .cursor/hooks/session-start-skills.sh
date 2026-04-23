#!/bin/bash
# Emits compact skill table as additional_context on session start.
# ~300 cached tokens. Ensures all skills are visible without reading each file.

SKILLS_DIR=".cursor/skills"
TABLE="Available skills (read SKILL.md when trigger matches):\n\n"
TABLE+="| Skill | Trigger |\n|---|---|\n"

for skill_dir in "$SKILLS_DIR"/*/; do
  skill_name=$(basename "$skill_dir")
  skill_file="$skill_dir/SKILL.md"

  [[ -f "$skill_file" ]] || continue

  # Extract trigger/description from frontmatter; handle YAML block scalars (>-)
  trigger=$(awk '
    /^---/{f=!f; next}
    f && /^(trigger|description):/ {
      val = $0; sub(/^(trigger|description):[[:space:]]*/, "", val)
      if (val ~ /^>/) { getline; sub(/^[[:space:]]+/, ""); print; exit }
      print val; exit
    }
  ' "$skill_file")

  [[ -z "$trigger" ]] && trigger="see SKILL.md"

  TABLE+="| \`$skill_name\` | $trigger |\n"
done

printf '%s' "{\"additional_context\": \"$(printf '%s' "$TABLE" | sed 's/"/\\"/g')\"}"
exit 0
