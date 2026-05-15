# SPDX-License-Identifier: AGPL-3.0-or-later

.PHONY: fmt fmt-check check test test-full build-release

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

check:
	cargo clippy --all-targets --no-default-features --features dev-fast -- -D warnings

test:
	cargo test --all --no-default-features --features dev-fast

test-full:
	cargo test --all --features full

build-release:
	cargo build --release --locked --bin kaizen --features full
