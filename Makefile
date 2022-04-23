SHELL:=/bin/bash

.DEFAULT_GOAL: build
.PHONY: fix fmt lint check build release test pre-commit install clean

fix:
	cargo fix --allow-staged --all-targets
	cargo clippy --all-targets --fix --allow-staged

fmt:
	cargo fmt --all

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings
	-cargo audit

check:
	cargo check

build:
	cross build

release:
	cross build --release

test:
	cargo test

pre-commit: lint test release

install:
	cargo install --force --path .

clean:
	cargo clean