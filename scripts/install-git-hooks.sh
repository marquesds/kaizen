#!/bin/sh
# Point this repo at version-controlled hooks under githooks/ (e.g. pre-commit rustfmt).
set -e
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
git config core.hooksPath githooks
printf '%s\n' "git: core.hooksPath set to githooks/ (run this once per clone)."
