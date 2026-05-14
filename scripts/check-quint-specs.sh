#!/bin/sh
# Fast Quint checks for specs/*.qnt (typecheck + repo invariants).
# Used in CI and optionally from pre-commit when .qnt files are staged.
set -e
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

if ! command -v quint >/dev/null 2>&1; then
	printf '%s\n' "check-quint-specs: quint not on PATH (see CONTRIBUTING.md)" >&2
	exit 1
fi

status=0
jobs="${QUINT_JOBS:-}"
if [ -z "$jobs" ]; then
	jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || printf '%s\n' 2)"
fi

case "$jobs" in
	''|*[!0-9]*|0) jobs=2 ;;
esac

tmpdir="$(mktemp -d)"
cleanup() {
	rm -rf "$tmpdir"
}
trap cleanup EXIT INT TERM

set -- specs/*.qnt
if [ -f "$1" ]; then
	printf '%s\0' "$@" |
		xargs -0 -n 1 -P "$jobs" sh -c '
			tmpdir="$0"
			f="$1"
			name="$(printf "%s" "$f" | sed "s#[^A-Za-z0-9_.-]#_#g")"
			printf "%s\n" "quint typecheck $f"
			quint typecheck "$f" || {
				: > "$tmpdir/$name.fail"
				exit 1
			}
		' "$tmpdir" || status=1
fi

if [ "$status" -ne 0 ]; then
	printf '%s\n' "check-quint-specs: one or more Quint typechecks failed" >&2
fi

# `init` must not be an arm of `action step`: simulation bootstraps via the default
# init transition only. Listing init in step allows reset from any state and can
# break quint run --mbt on some platforms.
if grep -nE '^[[:space:]]+init,[[:space:]]*$' specs/*.qnt 2>/dev/null; then
	printf '%s\n' "check-quint-specs: do not use 'init,' inside action step (compare specs/session-lifecycle.qnt)." >&2
	exit 1
fi

exit "$status"
