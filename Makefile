.PHONY: build lint lint-fix test format format-check install banner docs

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

# One-shot setup on a new machine: build, register the Claude Code hooks, and
# seed the deploy-status workflow if there is not one already.
install:
	cargo install --path . --locked
	$(HOME)/.cargo/bin/hotword install
	@mkdir -p $(HOME)/.config/hotword
	@test -f $(HOME)/.config/hotword/deploy-status.toml || cp examples/deploy-status.toml $(HOME)/.config/hotword/deploy-status.toml
	$(HOME)/.cargo/bin/hotword

# The banner PNG is rendered from site/banner.svg, never edited by hand.
banner:
	bun scripts/build-banner.mjs

# The docs pages under site/docs are rendered from the repo markdown.
docs:
	bun scripts/build-docs.mjs
