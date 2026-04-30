#!/usr/bin/env zsh
# Driver for docs/plans/kaizen-local-walkthrough.md (asciinema).
#
# Real Cursor/Claude sessions from the local store (not synthetic hooks). The driver omits `--refresh`
# on list/summary so recording stays fast; run `kaizen sessions list --refresh` in a TTY once before asciinema if needed.
# Writes experiments into .kaizen (exp new / tag / report are mutating).
#
# Env:
#   KAIZEN_BIN            — required absolute path to kaizen binary
#   KAIZEN_DEMO_ROOT      — workspace root (default: git root parent of this script)
#   KAIZEN_DEMO_ISOLATED=1 — use a temp KAIZEN_HOME only (machine.db); workspace .kaizen stays on disk
#   KAIZEN_DEMO_ALLOW_EMPTY=1 — skip exit when no sessions (debug only)
#   KAIZEN_DEMO_PAUSE_SEC — short pause between most steps (default 1.2)
#   KAIZEN_DEMO_READ_PAUSE_SEC — longer dwell after heavy output (tables, summary, metrics, retro, exp report; default 4.8)
#   KAIZEN_DEMO_REINDEX=1 — run `metrics index --force` and a second `metrics` (adds ~20s+)
#
# Record (zsh -f avoids ~/.zshrc aliases breaking this script):
#   KAIZEN_BIN="$PWD/target/debug/kaizen" asciinema rec --overwrite \
#     --window-size 100x30 -c "zsh -f $PWD/scripts/record-kaizen-walkthrough.sh" \
#     docs/plans/demo-wow.cast
#

emulate -R zsh
setopt err_exit pipe_fail

[[ -n ${KAIZEN_BIN:-} ]] || {
  print -r "set KAIZEN_BIN to absolute path of kaizen (e.g. target/debug/kaizen)" >&2
  exit 1
}
export KAIZEN_DAEMON=0

DEMO_PAUSE=${KAIZEN_DEMO_PAUSE_SEC:-1.2}
DEMO_READ_PAUSE=${KAIZEN_DEMO_READ_PAUSE_SEC:-4.8}

gray_stream() {
  emulate -L zsh
  local line pref=$'\e[38;5;244m' suf=$'\e[0m'
  while IFS= read -r line || [[ -n $line ]]; do
    print -r -- "${pref}${line}${suf}"
  done
}

run_kz() {
  emulate -L zsh
  local -a show
  show=(--no-daemon "$@")
  local -a parts
  local a
  for a in "${show[@]}"; do
    parts+=(${(q)a})
  done
  print -rP $'%B%F{cyan}$%f%b %F{cyan}kaizen '${(j: :)parts}$'%f'
  "$KAIZEN_BIN" "${show[@]}" 2>&1 | tee "$TMP/kaizen_last.out" | gray_stream
  return ${pipestatus[1]}
}

run_kz_capture() {
  emulate -L zsh
  local -a show
  show=(--no-daemon "$@")
  local -a parts
  local a
  for a in "${show[@]}"; do
    parts+=(${(q)a})
  done
  print -rP $'%B%F{cyan}$%f%b %F{cyan}kaizen '${(j: :)parts}$'%f'
  "$KAIZEN_BIN" "${show[@]}" >"$TMP/kaizen_last.out" 2>&1
  return $?
}

pause_short() { sleep "$DEMO_PAUSE"; }

pause_read() { sleep "$DEMO_READ_PAUSE"; }

TMP=""
trap '[[ -n $TMP ]] && rm -rf -- $TMP' EXIT

TMP=$(mktemp -d)
if [[ ${KAIZEN_DEMO_ISOLATED:-} == 1 ]]; then
  export KAIZEN_HOME=$TMP/kaizen-home
  mkdir -p "$KAIZEN_HOME"
fi

REPO_ROOT=${KAIZEN_DEMO_ROOT:-}
if [[ -z $REPO_ROOT ]]; then
  REPO_ROOT=$(cd -- "${0:A:h}/.." && pwd)
fi
cd -- "$REPO_ROOT" || exit 1

run_kz init
pause_short
run_kz doctor
pause_short

run_kz sessions list
pause_read

run_kz_capture sessions list --json --limit 1
LAST_OUT=$TMP/kaizen_last.out OUT_COUNT=$TMP/sess_count OUT_SID=$TMP/first_sid.txt python3 -c "
import json, os
d = json.load(open(os.environ['LAST_OUT']))
n = len(d['sessions'])
open(os.environ['OUT_COUNT'], 'w').write(str(n))
if d['sessions']:
    open(os.environ['OUT_SID'], 'w').write(d['sessions'][0]['id'] + '\n')
"
SESSION_COUNT=$(<$TMP/sess_count)

if [[ $SESSION_COUNT -eq 0 ]]; then
  if [[ ${KAIZEN_DEMO_ALLOW_EMPTY:-} == 1 ]]; then
    print -r >&2 "demo: 0 sessions — continuing; experiment steps will be skipped."
  else
    print -r "no sessions in this workspace — run kaizen sessions list --refresh in a TTY first, or use agents here; see kaizen doctor" >&2
    exit 1
  fi
fi

pause_short
run_kz summary
pause_read
run_kz_capture summary --json
LAST_OUT=$TMP/kaizen_last.out python3 -c "
import json, os
p = os.environ['LAST_OUT']
with open(p) as f:
    d = json.load(f)
s = json.dumps(d, indent=2)
print(s[:1200] + ('…' if len(s) > 1200 else ''))
" | gray_stream
pause_read

run_kz metrics --days 7
pause_read
if [[ ${KAIZEN_DEMO_REINDEX:-} == 1 ]]; then
  run_kz metrics index --force
  pause_short
  run_kz metrics --days 7
  pause_read
fi

run_kz retro --dry-run --days 7
pause_read

if [[ $SESSION_COUNT -gt 0 ]]; then
run_kz exp power --metric tokens_per_session --baseline-n 50
pause_short
run_kz exp new --name demo-rule \
  --hypothesis "repo rule cuts repeated shell failures" \
  --change "add a local smoke command and document env vars" \
  --metric tokens_per_session \
  --bind manual \
  --duration-days 14 --target-pct -10
LAST_OUT=$TMP/kaizen_last.out python3 -c "
import pathlib, os
s = pathlib.Path(os.environ['LAST_OUT']).read_text()
i = s.find(' · ')
assert s.startswith('created '), s
print(s[len('created '):i].strip())
" >"$TMP/exp_id.txt"
read -r EXP_ID <"$TMP/exp_id.txt"
pause_short
run_kz exp start "$EXP_ID"
pause_short
run_kz exp list
pause_short
run_kz exp status "$EXP_ID"
pause_short
read -r FIRST_SID <"$TMP/first_sid.txt"
run_kz exp tag "$EXP_ID" --session "$FIRST_SID" --variant treatment
pause_short
run_kz exp report "$EXP_ID"
pause_read
fi
