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
for f in specs/*.qnt; do
	[ -f "$f" ] || continue
	printf '%s\n' "quint typecheck $f"
	quint typecheck "$f" || status=1
done

# `init` must not be an arm of `action step`: simulation bootstraps via the default
# init transition only. Listing init in step allows reset from any state and can
# break quint run --mbt on some platforms.
if grep -nE '^[[:space:]]+init,[[:space:]]*$' specs/*.qnt 2>/dev/null; then
	printf '%s\n' "check-quint-specs: do not use 'init,' inside action step (compare specs/session-lifecycle.qnt)." >&2
	exit 1
fi

exit "$status"
