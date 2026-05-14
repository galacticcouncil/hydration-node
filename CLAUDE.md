# CLAUDE.md — hydration-node

## Protocol context

For Hydration protocol-level context (architecture, products, tokenomics, Omnipool mechanics), fetch the central context index via WebFetch:
`https://raw.githubusercontent.com/galacticcouncil/hydration/main/CLAUDE.md`

It lists available reference documents and their raw GitHub URLs.

## Shared AI skills

Repo-local AI skills live in `ai_skills/` so they can be used by multiple coding
agents, not only Claude.

When the user invokes a skill by name, or the task clearly matches a skill
description, load the corresponding `ai_skills/<skill-name>/SKILL.md` file and
follow its instructions. Resolve any relative paths in a skill from that skill's
directory.

Available shared skills:
- `hydration_cl0wdit` - security audit workflow for Substrate runtime and pallet
  code.

## Project overview

Substrate-based parachain (Polkadot ecosystem) implementing DeFi protocols — DEX (Omnipool, Stableswap, XYK, LBP), DCA, OTC, bonds, staking, governance, and EVM compatibility.

**Repo:** `galacticcouncil/hydration-node`
**Runtime:** `runtime/hydradx/` — current version 399.0.0
**Pallets:** 42+ custom pallets in `pallets/`
**Toolchain:** Rust 1.84.1, target `wasm32-unknown-unknown`

## Build & test

```sh
make build           # release build
make test            # cargo test --locked
make test-release    # cargo test --release --locked
make clippy          # clippy with -D warnings (RUSTFLAGS)
make format          # cargo fmt
make build-benchmarks  # build with runtime-benchmarks feature
make test-benchmarks   # test with runtime-benchmarks feature
```

Single pallet test: `cargo test -p pallet-omnipool --locked`

All cargo commands use `--config net.git-fetch-with-cli=true` (see Makefile).

## Test Naming Convention

Use BDD-style "should-when" naming for all tests. The test name should read as a specification of behavior.

**Format:** `<subject>_should_<expected_outcome>_when_<condition>`

**Examples:**
- `transfer_should_fail_when_balance_is_insufficient`
- `route_suggester_should_return_shortest_path_when_multiple_routes_exist`
- `sell_should_succeed_when_slippage_within_limit`
- `add_liquidity_should_fail_when_pool_is_frozen`

**Rules:**
- Use `snake_case` (Rust convention).
- For success cases, use `should_<outcome>_when_<condition>` (omit "succeed" if the outcome is descriptive enough).
- For failure cases, prefer `should_fail_when_<condition>` and assert on the specific error.
- The `<subject>` is typically the function/extrinsic under test.
- Avoid generic names like `test_1`, `it_works`, or `basic_test`.
- One behavior per test — if you need "and" in the name, split it into two tests.

## Extrinsic documentation

Every public extrinsic in `#[pallet::call]` blocks must have a rustdoc comment that follows
this standard structure. See `pallets/omnipool/src/lib.rs` and `pallets/stableswap/src/lib.rs`
for canonical examples.

**Required sections (in order):**

1. **Description** — one-line summary, then any longer explanation as additional paragraphs.
   Cover what the extrinsic does, important preconditions, and notable side effects (NFT
   minting, hooks, tradability flags, error conditions worth highlighting).
2. **Parameters** — a `Parameters:` block listing every argument as `` - `name`: description ``.
   Include `origin` when its required type is non-trivial (e.g. `T::AuthorityOrigin`).
3. **Emitted events** — a final line of the form `` Emits `EventName` event when successful. ``
   If multiple events are emitted, list each on its own line.

**Format:**

```rust
/// <One-line summary of what the extrinsic does.>
///
/// <Optional longer explanation: preconditions, side effects, error conditions,
/// hook invocations, tradability flags, etc. Use multiple paragraphs as needed.>
///
/// Parameters:
/// - `origin`: <only if origin type is non-trivial, e.g. Must be T::AuthorityOrigin>
/// - `param_a`: <what it represents and any constraints>
/// - `param_b`: <what it represents and any constraints>
///
/// Emits `SomethingHappened` event when successful.
///
#[pallet::call_index(N)]
#[pallet::weight(...)]
#[transactional]
pub fn my_extrinsic(...) -> DispatchResult { ... }
```

**Rules:**
- Use `///` doc comments (rustdoc), not `//` line comments.
- Blank `///` lines separate paragraphs and the three sections.
- Wrap identifiers, types, and values in backticks (e.g. `` `asset_id` ``, `` `T::AuthorityOrigin` ``).
- Phrase the emitted-events line consistently: `` Emits `X` event when successful. ``
- If the extrinsic delegates to another (e.g. `add_liquidity` → `add_liquidity_with_limit`),
  still document it in full — do not rely on the reader following the delegation.
- Keep parameter names in the doc identical to the function signature.

## Running tests

Dont't run tests with --release flag!

Do NOT prefix `cargo` commands with inline environment variables like 
`RUST_LOG=... cargo test`. Instead, export them first:

    export RUST_LOG=evm=error
    cargo test --locked -p runtime-integration-tests ...


## Code style

- **Tabs for indentation** (hard_tabs = true), max line width 120
- Trailing commas on multi-line lists, no trailing semicolons on last expressions
- **No `unwrap()` in production code** — require explicit proof comments (`// ... ; qed`) for `expect()`
- **No unsafe code** unless specifically permitted
- Indent depth > 5 is a smell — extract with `let` bindings or helper functions
- `where` clause indented one level, items one further
- Follow `rustfmt.toml` (edition 2021, reorder imports)

## Commit & PR conventions

**Conventional commits** — PR titles (and merge commits) must follow:
```
<type>(<scope>)<breaking>: <subject>
```

Types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `style`, `ci`, `build`
- Scope = affected pallet/module name (omit if multi-scope)
- Subject: imperative, lowercase, no period
- Breaking changes: add `!` after scope (e.g., `feat(claim)!: ...`)

**Branches:** `fix/description` or `feat/description`

## Versioning

- **SemVer** on all crates — bump `Cargo.toml` version on changes
- **Runtime:** bump `spec_version` for breaking changes, `impl_version` for non-breaking
- CI enforces version bumps — PRs will fail checks if versions aren't updated

## Project structure

```
pallets/              # 42+ custom pallets (omnipool, stableswap, xyk, lbp, dca, ...)
runtime/hydradx/      # Main runtime
  src/lib.rs          # construct_runtime!, recursion_limit = 512
  src/weights/        # 66+ auto-generated weight files — DO NOT hand-edit
  src/migrations.rs   # Storage migrations
  src/assets.rs       # Asset configuration
  src/xcm/            # Cross-chain messaging config
runtime/adapters/     # Runtime adapter layer
math/                 # hydra-dx-math library
primitives/           # Core types and traits
traits/               # hydradx-traits shared trait definitions
integration-tests/    # Full runtime integration tests
node/                 # Binary: CLI, RPC, service
precompiles/          # EVM precompiles (call-permit, flash-loan)
scripts/              # Benchmarking and deployment scripts
launch-configs/       # Zombienet, Chopsticks, fork configs
```

## Pallet structure (standard layout)

```
pallets/<name>/
  src/lib.rs           # Pallet logic (#[pallet::call], storage, events, errors)
  src/weights.rs       # Benchmarked weights (auto-generated)
  src/benchmarking.rs  # Benchmark definitions
  src/tests/           # Unit tests
    mock.rs            # Mock runtime setup
    mod.rs             # Test module organization
    *.rs               # Test files per feature (buy.rs, sell.rs, etc.)
  Cargo.toml
```

## Key patterns

- **Weights are auto-generated** from benchmarks — never edit `weights.rs` files by hand. Use `scripts/benchmarking.sh` or add `[ignore benchmarks]` to PR title to skip.
- **Math-heavy code** lives in `math/` crate with `fixed` and `rug` for arbitrary precision. Pallets call into math functions — keep math logic separate from storage/dispatch logic.
- **ORML integration** for multi-currency support (orml-tokens, orml-currencies).
- **Frontier/EVM** integration for Ethereum compatibility — EVM-related pallets include `dynamic-evm-fee`, `evm-accounts`, and precompiles.
- **XCM** for cross-chain asset transfers — config in `runtime/hydradx/src/xcm/`.
- **Circuit breaker** pallet for risk management — limits large trades/liquidity changes.

## Testing guidelines

- All features and bug fixes **must have tests**
- Unit tests go in `pallets/<name>/src/tests/`
- Integration tests in `integration-tests/` run against the full runtime
- Mock runtimes: each pallet has its own `mock.rs`; `runtime-mock/` for shared mocking
- Property testing available via `proptest`

## Dependencies

- **Polkadot SDK fork:** `galacticcouncil/polkadot-sdk` branch `polkadot-stable2503-11-patch2`
- **ORML fork:** `bifrost-io/open-runtime-module-library` branch `polkadot-stable2503`
- Codec: `parity-scale-codec 3.7`
- All deps managed at workspace level in root `Cargo.toml`

## CI checks

CI runs on every PR:
1. `cargo fmt --check`
2. `clippy --release --all-targets` (warnings = errors)
3. `test --release`
4. Benchmark build check
5. Semantic PR title validation
6. Version bump validation
