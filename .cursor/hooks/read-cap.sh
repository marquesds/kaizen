#!/bin/bash
# beforeReadFile hook — blocks unsliced reads of large files.
# Deny if content line count exceeds threshold and no offset/limit was used.
# Threshold: 150 lines general; 300 for .md files; exempt: lockfiles.

command -v jq >/dev/null 2>&1 || { echo '{"permission":"allow"}'; exit 0; }

input=$(cat)
file_path=$(printf '%s' "$input" | jq -r '.file_path // ""')
content=$(printf '%s' "$input" | jq -r '.content // ""')

[[ -z "$content" ]] && { echo '{"permission":"allow"}'; exit 0; }

# Lockfiles — exempt from cap
case "$file_path" in
  */mix.lock|*/package-lock.json|*/yarn.lock|*/Cargo.lock|*/poetry.lock)
    echo '{"permission":"allow"}'
    exit 0
    ;;
esac

line_count=$(printf '%s' "$content" | wc -l | tr -d ' ')

# .md files get higher threshold
threshold=150
case "$file_path" in
  *.md|*.mdc) threshold=300 ;;
esac

if (( line_count > threshold )); then
  msg="Read $line_count lines from $(basename "$file_path") without offset\\/limit. Use Read with offset+limit, or Grep for signature scan first."
  printf '{"permission":"deny","user_message":"%s"}' "$msg"
  exit 0
fi

echo '{"permission":"allow"}'
