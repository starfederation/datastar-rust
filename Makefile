.PHONY: all

all:
	@echo "Usage:"
	@echo "fmt                           - run the rust formatter"
	@echo "sort                          - sort TOML dependencies"
	@echo "lint                          - combine fmt+sort"
	@echo "check                         - shallow check of rust code (pre-compile)"
	@echo "clippy                        - run clippy checks"
	@echo "doc                           - doc checks"
	@echo "hack                          - test feature matrix compatibility"
	@echo "test                          - run all unit and doc tests"
	@echo "qa                            - combine lint+check+clippy+doc+hack+test"
	@echo "detect-unused-deps            - detect unused deps for removal"
	@echo "hello-axum                    - run hello-world example using the Axum framework"
	@echo "activity-feed-axum            - run activity-feed example using the Axum framework"
	@echo "test-suite-axum               - run test-suite example runner using the Axum framework"
	@echo "hello-rocket                  - run hello-world example using the Rocket framework"
	@echo "hello-channel-rocket          - run hello-world w/ a channel example using the Rocket framework"
.PHONY:

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

sort:
	cargo sort --grouped

sort-check:
	cargo sort --workspace --grouped --check

lint: fmt sort

check:
	cargo check --all-targets --all-features

clippy:
	cargo clippy --all-targets --all-features

doc:
	RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc --all-features --no-deps

hack:
	cargo hack check --each-feature --no-dev-deps

test:
	cargo test --all-features

qa: lint check clippy doc test

detect-unused-deps:
	# https://github.com/bnjbvr/cargo-machete
	cargo machete --skip-target-dir

update-deps:
	cargo upgrade
	cargo upgrades
	cargo update

hello-axum:
	cargo run --example axum-hello --features axum,tracing

activity-feed-axum:
	cargo run --example axum-activity-feed --features axum,tracing

test-suite-axum:
	cargo run --example axum-test-suite --features axum,tracing

hello-rocket:
	cargo run --example rocket-hello --features rocket

hello-channel-rocket:
	cargo run --example rocket-hello-channel --features rocket
