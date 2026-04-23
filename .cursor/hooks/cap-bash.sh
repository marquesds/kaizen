#!/bin/bash
# Gates wasteful shell patterns. Works for Cursor and Claude Code.
# Fails open if jq missing. Starts with "ask" — tune to deny after thresholds confirmed.

command -v jq >/dev/null 2>&1 || { echo '{"permission":"allow"}'; exit 0; }

input=$(cat)

# Detect agent by input shape:
#   Cursor:      {"command": "..."}
#   Claude Code: {"tool_input": {"command": "..."}, "tool_name": "Bash", ...}
if printf '%s' "$input" | jq -e '.tool_input' >/dev/null 2>&1; then
  AGENT="claude"
  cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty')
else
  AGENT="cursor"
  cmd=$(printf '%s' "$input" | jq -r '.command // empty')
fi

[[ -z "$cmd" ]] && { echo '{"permission":"allow"}'; exit 0; }

deny() {
  local msg="$1"
  if [[ "$AGENT" == "claude" ]]; then
    printf '{"decision":"block","reason":"%s"}' "$msg"
  else
    printf '{"permission":"ask","user_message":"%s","agent_message":"Hook: %s"}' "$msg" "$msg"
  fi
  exit 0
}

allow() {
  if [[ "$AGENT" == "claude" ]]; then
    echo '{"decision":"allow"}'
  else
    echo '{"permission":"allow"}'
  fi
  exit 0
}

# cat on large files
if printf '%s' "$cmd" | grep -qE '^\s*cat\s+'; then
  file=$(printf '%s' "$cmd" | grep -oE 'cat\s+\S+' | awk '{print $2}' | head -1)
  if [[ -f "$file" ]]; then
    lines=$(wc -l < "$file" 2>/dev/null || echo 0)
    if (( lines > 200 )); then
      deny "cat on $file ($lines lines) — use Read tool with offset/limit instead"
    fi
  fi
fi

# rg without result cap
if printf '%s' "$cmd" | grep -qE '^\s*rg\s+'; then
  if ! printf '%s' "$cmd" | grep -qE '(-m\s|--max-count|\| head)'; then
    deny "uncapped rg — use Grep tool with head_limit param"
  fi
fi

# find without depth/result limit
if printf '%s' "$cmd" | grep -qE '^\s*find\s+'; then
  if ! printf '%s' "$cmd" | grep -qE '(-maxdepth|\| head|\| tail)'; then
    deny "unbounded find — use Glob tool for recursive file matching"
  fi
fi

# head/tail with large line count
if printf '%s' "$cmd" | grep -qE '^\s*(head|tail)\s+'; then
  n=$(printf '%s' "$cmd" | grep -oE '\-n\s*[0-9]+' | grep -oE '[0-9]+' | head -1)
  if [[ -n "$n" ]] && (( n > 500 )); then
    deny "head/tail -n $n (>500) — use Read tool with offset/limit"
  fi
fi

allow
