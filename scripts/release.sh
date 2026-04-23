#!/usr/bin/env bash
# kaizen release: bump version, tag, cross-compile, publish.
# Usage: scripts/release.sh <new-version>
#
# Requires: cargo-edit (cargo set-version), cross, git, gh, crates.io token.
# Aborts on dirty tree or failing pre-release checks.

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <new-version>" >&2
  exit 64
fi
VERSION="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree dirty; commit or stash first" >&2
  exit 1
fi

echo ">> cargo fmt --check"
cargo fmt --all -- --check
echo ">> cargo clippy -D warnings"
cargo clippy --all-targets -- -D warnings
echo ">> cargo test"
cargo test --all
echo ">> cargo deny check"
cargo deny --manifest-path Cargo.toml --config .cargo/deny.toml check

echo ">> bump version -> ${VERSION}"
cargo set-version "${VERSION}"

git add Cargo.toml Cargo.lock
git commit -m "release: ${VERSION}"
git tag -s "v${VERSION}" -m "kaizen ${VERSION}"

echo ">> cross-compile"
for TARGET in x86_64-apple-darwin aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  echo "   - $TARGET"
  cross build --release --target "$TARGET" --bin kaizen
done

echo ">> cargo publish"
cargo publish --locked

echo ">> git push"
git push origin HEAD
git push origin "v${VERSION}"

echo "done — kaizen ${VERSION} released"
