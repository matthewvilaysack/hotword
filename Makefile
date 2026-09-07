.PHONY: build lint lint-fix test format format-check

build:
	cargo build --release

lint:
	cargo clippy --all-targets -- -D warnings

lint-fix:
	cargo clippy --all-targets --fix --allow-dirty

test:
	cargo test

format:
	cargo fmt

format-check:
	cargo fmt --check
