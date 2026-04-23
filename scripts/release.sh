#!/usr/bin/env bash
# kaizen release: run pre-flight checks, bump version, tag, push.
# GitHub Actions (.github/workflows/release.yml) picks up the tag and
# cross-compiles, uploads the release, and publishes to crates.io.
#
# Usage: scripts/release.sh <new-version>
#
# Requires: cargo-edit (cargo set-version), git, gh.
# Aborts on dirty tree or failing pre-release checks.

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <new-version>" >&2
  exit 64
fi
VERSION="$1"
TAG="v${VERSION}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "error: working tree dirty; commit or stash first" >&2
  exit 1
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" && "$BRANCH" != "master" ]]; then
  echo "error: release from main/master only (current: $BRANCH)" >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  echo "error: tag ${TAG} already exists" >&2
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

if ! grep -q "^## \[${VERSION}\]" CHANGELOG.md; then
  echo "warning: no '## [${VERSION}]' section in CHANGELOG.md" >&2
  read -r -p "continue anyway? [y/N] " ans
  [[ "$ans" == "y" || "$ans" == "Y" ]] || exit 1
fi

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: ${VERSION}"
git tag -s "${TAG}" -m "kaizen ${VERSION}"

echo ">> git push origin ${BRANCH} ${TAG}"
git push origin "${BRANCH}"
git push origin "${TAG}"

echo "done — tag ${TAG} pushed. CI will build binaries + publish."
echo "watch: gh run watch"
