.PHONY: build
build:
	cargo build --release
	ln -f $(CURDIR)/target/release/hydradx $(CURDIR)/target/release/testing-hydradx

.PHONY: check
check:
	cargo check --release

.PHONY: build-benchmarks
build-benchmarks:
	cargo build --release --features runtime-benchmarks

.PHONY: test
test:
	cargo test --release

.PHONY: test-benchmarks
test-benchmarks:
	cargo test --release --features runtime-benchmarks

.PHONY: coverage
coverage:
	cargo tarpaulin --avoid-cfg-tarpaulin --all-features --workspace --locked  --exclude-files node/* --exclude-files runtime/* --exclude-files infrastructure/*  --exclude-files utils/* --exclude-files **/weights.rs --ignore-tests -o Xml -o lcov

.PHONY: clippy
clippy:
	cargo clippy --release --all-targets --all-features -- -D warnings -A deprecated


.PHONY: format
format:
	cargo fmt

.PHONY: build-docs
build-docs:
	cargo doc --release --target-dir ./HydraDX-dev-docs --no-deps

.PHONY: clean
clean:
	cargo clean

.PHONY: docker
docker:
	docker build -t hydra-dx .
