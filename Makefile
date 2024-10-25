cargo := cargo --config net.git-fetch-with-cli=true
ifeq ($(shell uname),Darwin)
    # macOS-specific commands
    sha256sum := shasum -a 256
else
    # Default commands for other systems
    sha256sum := sha256sum
endif

.PHONY: build
build:
	$(cargo) build --release --locked

.PHONY: build-release
build-release:
	$(cargo) build --release --locked --features metadata-hash

.PHONY: check
check:
	$(cargo) check --release

.PHONY: build-benchmarks
build-benchmarks:
	$(cargo) build --release --features runtime-benchmarks

.PHONY: test
test:
	$(cargo) test --locked

.PHONY: test-release
test-release:
	$(cargo) test --release --locked

.PHONY: test-benchmarks
test-benchmarks:
	$(cargo) test --release --features runtime-benchmarks

.PHONY: coverage
coverage:
	$(cargo) tarpaulin --avoid-cfg-tarpaulin --all-features --workspace --locked  --exclude-files node/* --exclude-files runtime/* --exclude-files infrastructure/*  --exclude-files utils/* --exclude-files **/weights.rs --ignore-tests -o Xml -o lcov --timeout 120

.PHONY: clippy
clippy:
	$(cargo) clippy --release --locked --all-targets -- -D warnings -A deprecated

.PHONY: clippy-all
clippy-all:
	$(cargo) clippy --release --locked --all-targets --all-features -- -D warnings -A deprecated

.PHONY: format
format:
	$(cargo) fmt

.PHONY: try-runtime
try-runtime:
	$(cargo) build --release --features try-runtime
	try-runtime --runtime ./target/release/wbuild/hydradx-runtime/hydradx_runtime.wasm on-runtime-upgrade --blocktime 12000 --checks all live --uri wss://archive.rpc.hydration.cloud

.PHONY: build-docs
build-docs:
	$(cargo) doc --release --target-dir ./HydraDX-dev-docs --no-deps

.PHONY: clean
clean:
	$(cargo) clean

.PHONY: docker
docker:
	docker build -t hydra-dx .
	docker tag hydra-dx galacticcouncil/hydra-dx:latest

checksum:
	$(sha256sum) target/release/hydradx > target/release/hydradx.sha256
	cp target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm target/release/
	$(sha256sum) target/release/hydradx_runtime.compact.compressed.wasm > target/release/hydradx_runtime.compact.compressed.wasm.sha256

release: build-release checksum

all: clippy build-benchmarks test-benchmarks test build checksum

chopstics: release
	npx @acala-network/chopsticks xcm --parachain=launch-configs/chopsticks/hydradx.yml --parachain=launch-configs/chopsticks/assethub.yml
