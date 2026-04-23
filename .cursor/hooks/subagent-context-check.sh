#!/bin/bash
# subagentStart hook — requires <context_summary> block in large generalPurpose tasks.
# explore and shell types are soft-allowed (no block).

command -v jq >/dev/null 2>&1 || { echo '{"permission":"allow"}'; exit 0; }

input=$(cat)
subagent_type=$(printf '%s' "$input" | jq -r '.subagent_type // ""')
task=$(printf '%s' "$input" | jq -r '.task // ""')

# Only enforce on generalPurpose
[[ "$subagent_type" != "generalPurpose" ]] && { echo '{"permission":"allow"}'; exit 0; }

task_len=${#task}

# Short tasks don't need a summary
(( task_len <= 500 )) && { echo '{"permission":"allow"}'; exit 0; }

# Check for <context_summary> block
if printf '%s' "$task" | grep -q '<context_summary>'; then
  echo '{"permission":"allow"}'
  exit 0
fi

msg="generalPurpose subagent prompt ($task_len chars) missing <context_summary> block. Add: <context_summary>file → purpose → key lines<\\/context_summary>"
printf '{"permission":"deny","user_message":"%s"}' "$msg"
