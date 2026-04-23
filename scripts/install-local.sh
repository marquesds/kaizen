#!/usr/bin/env sh
# Install or reinstall `kaizen` from this working tree into ~/.cargo/bin (same as `cargo install --path .`).
# Run from anywhere:  ./scripts/install-local.sh   or   sh scripts/install-local.sh
set -e
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cargo install --path . --locked --force
if command -v kaizen >/dev/null 2>&1; then
	printf '%s\n' "kaizen -> $(command -v kaizen)"
else
	printf '%s\n' "Installed to ${CARGO_HOME:-$HOME/.cargo}/bin/kaizen (ensure that directory is on PATH)."
fi
