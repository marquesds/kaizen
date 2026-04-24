#!/bin/sh
set -eu

version="${QUINT_EVALUATOR_VERSION:-v0.6.0}"
home_dir="${QUINT_HOME:-$HOME/.quint}"
dest_dir="$home_dir/rust-evaluator-$version"
dest_bin="$dest_dir/quint_evaluator"
tag="evaluator%2F$version"

if [ -x "$dest_bin" ]; then
	printf '%s\n' "quint evaluator already present: $dest_bin"
	exit 0
fi

os="$(uname -s)"
arch="$(uname -m)"

case "$os/$arch" in
	Darwin/arm64) asset="quint_evaluator-aarch64-apple-darwin.tar.gz" ;;
	Darwin/x86_64) asset="quint_evaluator-x86_64-apple-darwin.tar.gz" ;;
	Linux/aarch64) asset="quint_evaluator-aarch64-unknown-linux-gnu.tar.gz" ;;
	Linux/x86_64) asset="quint_evaluator-x86_64-unknown-linux-gnu.tar.gz" ;;
	*)
		printf '%s\n' "unsupported Quint evaluator target: $os/$arch" >&2
		exit 1
		;;
esac

url="https://github.com/informalsystems/quint/releases/download/$tag/$asset"
tmpdir="$(mktemp -d)"
archive="$tmpdir/$asset"

cleanup() {
	rm -rf "$tmpdir"
}

trap cleanup EXIT INT TERM

mkdir -p "$dest_dir"
printf '%s\n' "installing Quint evaluator $version for $os/$arch"
curl --fail --location --silent --show-error "$url" --output "$archive"
tar -xzf "$archive" -C "$dest_dir"
chmod +x "$dest_bin"
printf '%s\n' "installed Quint evaluator: $dest_bin"
