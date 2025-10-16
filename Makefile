.PHONY: clippy lint fmt-check deny

clippy:
	cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic -W clippy::nursery \
		-A clippy::module_name_repetitions -A clippy::implicit_hasher

lint: fmt-check clippy deny

fmt-check:
	cargo fmt --all -- --check

deny:
	cargo deny check licenses bans sources
