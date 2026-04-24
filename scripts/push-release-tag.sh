#!/usr/bin/env bash
# Resolve the next vMAJOR.MINOR.PATCH from Cargo.toml, bumping the patch
# if an existing tag points at a different commit (same as CI "push release tag").
#
# Real run: fetches tags, creates/pushes the tag, dispatches the Release
# workflow when GITHUB_ACTIONS is set.
#
# Usage:
#   scripts/push-release-tag.sh
#   scripts/push-release-tag.sh --dry-run
#   scripts/push-release-tag.sh --self-test
#
# Optional env: KIZEN_ROOT (default: repo root), CARGO_TOML, BUMP_MAX (default 256)

set -euo pipefail

bump_patch() {
  IFS=. read -r a b c <<< "$1" || return 1
  c=$((c + 1))
  echo "$a.$b.$c"
}

read_cargo_version() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "Cargo.toml not found: $file" >&2
    return 1
  fi
  awk -F'"' '/^version = / {print $2; exit}' "$file"
}

# Prints chosen tag to stdout as the only line:  RELEASE_TAG=v0.1.1
resolve_and_apply_release_tag() {
  local dry_run="$1"
  local version head_sha
  local v bumps cand tag_sha
  local push_url

  version="$(read_cargo_version "$CARGO_TOML")"
  if ! echo "$version" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "Cargo.toml version must be MAJOR.MINOR.PATCH, got: $version" >&2
    return 1
  fi
  head_sha=$(git -C "$KIZEN_ROOT" rev-parse HEAD)
  if [[ "$dry_run" = 1 ]]; then
    echo "[dry-run] KIZEN_ROOT=$KIZEN_ROOT HEAD=$head_sha version=$version" >&2
  else
    git -C "$KIZEN_ROOT" fetch --force --tags origin
  fi

  v="$version"
  bumps=0
  while true; do
    cand="v${v}"
    if ! git -C "$KIZEN_ROOT" rev-parse "${cand}^{commit}" >/dev/null 2>&1; then
      if [[ "$dry_run" = 1 ]]; then
        echo "[dry-run] would: git tag $cand $head_sha && git push <url> $cand" >&2
        echo "RELEASE_TAG=$cand"
        return 0
      fi
      echo "Tagging $cand at $head_sha" >&2
      push_url="https://${RELEASE_PUSH_USER:?}:${RELEASE_PUSH_TOKEN:?}@github.com/${GITHUB_REPOSITORY:?}.git"
      git -C "$KIZEN_ROOT" tag "$cand" "$head_sha"
      git -C "$KIZEN_ROOT" push "$push_url" "$cand"
      echo "RELEASE_TAG=$cand"
      return 0
    fi
    tag_sha=$(git -C "$KIZEN_ROOT" rev-parse "${cand}^{commit}")
    if [[ "$tag_sha" = "$head_sha" ]]; then
      echo "$cand already points at $head_sha" >&2
      echo "RELEASE_TAG=$cand"
      return 0
    fi
    if [[ "$bumps" -ge "$BUMP_MAX" ]]; then
      echo "More than $BUMP_MAX patch bumps; refusing to keep searching" >&2
      return 1
    fi
    echo "$cand already points at $tag_sha; trying patch bump from $v" >&2
    bumps=$((bumps + 1))
    v=$(bump_patch "$v")
  done
}

dispatch_release_if_needed() {
  local release_tag="$1"
  if [[ -z "${GITHUB_ACTIONS:-}" ]]; then
    echo "Not in GitHub Actions; skipping workflow dispatch" >&2
    return 0
  fi
  if ! command -v gh &>/dev/null; then
    echo "gh not in PATH; cannot check or back up-dispatch Release" >&2
    return 0
  fi
  # Tag push normally triggers the Release workflow via on: push: tags. Some setups still miss it.
  # gh run list --branch <tag> is unreliable: tag runs often have headBranch != tag name, so
  # we match any Release run whose headSha equals the tag's target commit.
  local tag_sha found
  if ! tag_sha=$(git -C "$KIZEN_ROOT" rev-parse "refs/tags/${release_tag}^{commit}" 2>/dev/null); then
    echo "no ref refs/tags/${release_tag}; cannot match workflow runs" >&2
    return 0
  fi
  for _round in 1 2 3 4 5 6; do
    if ! found=$(gh run list -R "$GITHUB_REPOSITORY" -w Release -L 30 \
      --json headSha,status,conclusion \
      --jq -r --arg s "$tag_sha" \
      '[.[] | select(.headSha == $s)] | length' 2>/dev/null); then
      found=0
    fi
    if [[ -n "$found" && "$found" -ge 1 ]]; then
      echo "Release run(s) for $release_tag (commit $tag_sha) already in Actions: $found" >&2
      return 0
    fi
    if [[ "$_round" -lt 6 ]]; then
      sleep 5
    fi
  done
  echo "No Release run for $release_tag@$tag_sha after ~30s; dispatching (backup for missed tag push trigger)" >&2
  # gh run URL to stderr; keep stdout clean for eval "$(bash …)" in CI.
  gh workflow run Release \
    -R "$GITHUB_REPOSITORY" \
    -f version="${release_tag#v}" \
    --ref "$release_tag" >&2
}

run_self_test() {
  local fails=0 want got tmp line name
  # Mirror resolve loop (kept in sync with resolve_and_apply_release_tag)
  expect_line() {
    local version="$1" head="$2" git_dir="$3"
    local v bumps=0 cand tag_sha
    v="$version"
    bumps=0
    while true; do
      cand="v${v}"
      if ! git -C "$git_dir" rev-parse "${cand}^{commit}" >/dev/null 2>&1; then
        echo "RELEASE_TAG=$cand"
        return 0
      fi
      tag_sha=$(git -C "$git_dir" rev-parse "${cand}^{commit}")
      if [[ "$tag_sha" = "$head" ]]; then
        echo "RELEASE_TAG=$cand"
        return 0
      fi
      if [[ "$bumps" -ge 256 ]]; then
        return 1
      fi
      bumps=$((bumps + 1))
      v=$(bump_patch "$v")
    done
  }

  echo "== push-release-tag self-test (3 scenarios) ==" >&2

  # A: no tags -> v0.1.0
  tmp=$(mktemp -d)
  (
    cd "$tmp"
    git -c init.defaultBranch=main init -q
    cat > Cargo.toml <<'EOF'
[package]
name = "t"
version = "0.1.0"
edition = "2021"
EOF
    echo a >f && git add f Cargo.toml && git -c user.email=t@e -c user.name=t commit -m one -q
  )
  name="no prior tags"
  want="v0.1.0"
  head=$(git -C "$tmp" rev-parse HEAD)
  line=$(expect_line "0.1.0" "$head" "$tmp")
  got="${line#RELEASE_TAG=}"
  if [[ "$got" != "$want" ]]; then
    echo "FAIL $name: want $want got $got" >&2
    fails=$((fails + 1))
  else
    echo "ok  $name -> $got" >&2
  fi
  rm -rf "$tmp"

  # B: v0.1.0 on first commit, HEAD second -> v0.1.1
  tmp=$(mktemp -d)
  (
    cd "$tmp"
    git -c init.defaultBranch=main init -q
    cat > Cargo.toml <<'EOF'
[package]
name = "t"
version = "0.1.0"
edition = "2021"
EOF
    echo a >f && git add f Cargo.toml && git -c user.email=t@e -c user.name=t commit -m one -q
    git tag v0.1.0
    echo b >f && git add f && git -c user.email=t@e -c user.name=t commit -m two -q
  )
  name="v0.1.0 on older commit"
  want="v0.1.1"
  head=$(git -C "$tmp" rev-parse HEAD)
  line=$(expect_line "0.1.0" "$head" "$tmp")
  got="${line#RELEASE_TAG=}"
  if [[ "$got" != "$want" ]]; then
    echo "FAIL $name: want $want got $got" >&2
    fails=$((fails + 1))
  else
    echo "ok  $name -> $got" >&2
  fi
  rm -rf "$tmp"

  # C: v0.1.0 on HEAD -> v0.1.0
  tmp=$(mktemp -d)
  (
    cd "$tmp"
    git -c init.defaultBranch=main init -q
    cat > Cargo.toml <<'EOF'
[package]
name = "t"
version = "0.1.0"
edition = "2021"
EOF
    echo a >f && git add f Cargo.toml && git -c user.email=t@e -c user.name=t commit -m one -q
    h=$(git rev-parse HEAD)
    git tag v0.1.0 "$h"
  )
  name="v0.1.0 already on HEAD"
  want="v0.1.0"
  head=$(git -C "$tmp" rev-parse HEAD)
  line=$(expect_line "0.1.0" "$head" "$tmp")
  got="${line#RELEASE_TAG=}"
  if [[ "$got" != "$want" ]]; then
    echo "FAIL $name: want $want got $got" >&2
    fails=$((fails + 1))
  else
    echo "ok  $name -> $got" >&2
  fi
  rm -rf "$tmp"

  # D: end-to-end: same script, --dry-run on fixture B
  tmp=$(mktemp -d)
  (
    cd "$tmp"
    git -c init.defaultBranch=main init -q
    cat > Cargo.toml <<'EOF'
[package]
name = "t"
version = "0.1.0"
edition = "2021"
EOF
    echo a >f && git add f Cargo.toml && git -c user.email=t@e -c user.name=t commit -m one -q
    git tag v0.1.0
    echo b >f && git add f && git -c user.email=t@e -c user.name=t commit -m two -q
  )
  name="script --dry-run on fixture B"
  want="v0.1.1"
  out=$(KIZEN_ROOT="$tmp" CARGO_TOML="$tmp/Cargo.toml" "$0" --dry-run 2>/dev/null) || out=""
  got="${out#RELEASE_TAG=}"
  if [[ "$got" != "$want" ]]; then
    echo "FAIL $name: want $want got ($out)" >&2
    fails=$((fails + 1))
  else
    echo "ok  $name -> $got" >&2
  fi
  rm -rf "$tmp"

  if [[ "$fails" -ne 0 ]]; then
    echo "self-test: $fails check(s) failed" >&2
    return 1
  fi
  echo "self-test: all passed" >&2
}

# --- main ---
SELFDIR="$(cd "$(dirname "$0")" && pwd)"
KIZEN_ROOT="${KIZEN_ROOT:-$(cd "$SELFDIR/.." && pwd)}"
CARGO_TOML="${CARGO_TOML:-$KIZEN_ROOT/Cargo.toml}"
BUMP_MAX="${BUMP_MAX:-256}"

case "${1:-}" in
  --self-test)
    run_self_test
    exit $?
    ;;
  --help | -h)
    sed -n '2,18p' "$0"
    exit 0
    ;;
  --dry-run)
    line=$(resolve_and_apply_release_tag 1)
    # stdout is only RELEASE_TAG= from the function; stderr has [dry-run] lines
    echo "$line"
    exit 0
    ;;
  "")
    if [[ -z "${RELEASE_PUSH_USER:-}" || -z "${RELEASE_PUSH_TOKEN:-}" || -z "${GITHUB_REPOSITORY:-}" ]]; then
      echo "error: need RELEASE_PUSH_USER, RELEASE_PUSH_TOKEN, GITHUB_REPOSITORY for a real run (or use --dry-run)" >&2
      exit 1
    fi
    line=$(resolve_and_apply_release_tag 0)
    rel="${line#RELEASE_TAG=}"
    dispatch_release_if_needed "$rel"
    echo "$line"
    exit 0
    ;;
  *)
    echo "usage: $0 [--dry-run | --self-test]" >&2
    exit 2
    ;;
esac
