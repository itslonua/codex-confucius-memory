.PHONY: check build
check:
	cargo fmt --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked
build:
	cargo build --release --locked
