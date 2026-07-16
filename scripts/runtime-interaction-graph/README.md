# Runtime interaction graph

Generates an evidence-backed interaction graph of Hydration's FRAME pallets, runtime configuration,
EVM subsystem, precompiles, token backends, storage, lifecycle functions, and dynamic callbacks.

```sh
python3 scripts/runtime-interaction-graph/runtime_interaction_graph.py
```

Outputs are written to `target/runtime-interaction-graph/` and are not committed:

- `interaction-graph.json` — machine-readable evidence graph
- `interaction-graph.dot` — Graphviz input
- `component-graph.json` — execution-only component edges, cycles, bounded paths, and truncation metadata
- `projections/*.json` — raw-inventory execution, callback, configuration, state, asset, authorization,
  EVM-interface, and deployment graphs
- `projections-active/*.json` — the operational projections, excluding explicit and inherited inactive nodes
  while retaining unclassified evidence
- `audit-candidates.md` — functions combining state access and external execution
- `interaction-cycles.md` — cross-component cycles, with EVM/precompile cycles prioritized
- `execution-boundaries.md` — native↔EVM and precompile→FRAME dispatch sites
- `execution-paths.md` — bounded multi-component paths ending at EVM or FRAME boundaries
- `audit-overview.dot` — audit-focused component, callback, boundary, and asset view
- `audit-overview.svg` — deterministic dependency-free rendered audit overview
- `graph-scale.svg` — bounded executive dashboard of raw/operational totals, tri-state activity, top distributions,
  and evidence provenance
- `dangerous-interactions.md` — cross-domain and state/call-order rule matches
- `focused/*.svg` — cycle/path views plus component, execution-boundary, router, and token-flow layers
- `semantic-coverage.md` — per-package RAPx callgraph, MIR, and dataflow status
- `mir-coverage.md` — functions and relevant operations imported from genuine rustc MIR
- `graph-coverage.md` — node, edge, component, and unresolved-target coverage totals
- `resolution-coverage.md` — unresolved and inventory-only targets grouped by evidence-backed reason
- `coverage.json` — CI metrics checked against `coverage-thresholds.json`
- `integration-test-coverage.{json,md}` — graph components, entrypoints, and interactions linked from integration tests
- `query-packs.json` — reusable cycles, paths, ordering, origin, lifecycle, token, contract, and entrypoint queries
- `interaction-graph.html` — dependency-free clickable canvas explorer with projection, domain, edge-kind, search,
  pan/zoom, neighbor highlighting, evidence samples, deep links, and bounded API drill-down
- `completeness.{json,md}` — source-component, entrypoint-class, MIR, state, asset, and contract inventory
- `prioritized-test-gaps.md` — uncovered static targets ranked by privilege, asset impact, and domain crossings
- `focused/state-invariants.svg` — storage, ledgers, pool state, guards, and source-validated invariants
- `evm-build-provenance.json` — hashes and tool, repository, chain, block, and optional semantic-input provenance

Edges are evidence-bearing and conservative. Associated types resolve by canonical Rust trait path,
runtime instance, import alias, and declared supertraits. Qualified `<T as Trait>::Type` calls, grouped
`use` imports, aliased crates, and generic associated-type assignments retain their exact identity; the
graph does not infer targets from a same-named associated type in another trait or pallet instance.

Associated types configured by `impl pallet_*::Config for Runtime` resolve to either a mapped
component or an exact runtime-binding node with `resolution: runtime-config`. Trait-bound `Get`
and weight providers are classified as configuration reads, not callbacks. Compiler-derived RAPx
and rustc MIR extend this source evidence without replacing it. MIR receivers retain their exact source module,
Config trait, and pallet instance. Source-only pallets, unimplemented nested
`Config` traits, and unconfigured migrations remain in the raw inventory with `runtime_active: false` and
an evidence-backed reason. They remain visible under `projections/`, but are excluded from
`projections-active/`, operational query packs, component/path/cycle graphs, integration-test coverage, and
focused token/state visualizations. Coverage requires zero unresolved runtime targets, enforces operational projection
floors separately from raw inventory totals, and reports inventory-only targets separately.

The scale dashboard uses three activity states: `active` requires explicit `runtime_active: true`, `inactive`
includes explicit `false` and owner/source-function inheritance, and missing affirmative metadata remains
`unclassified`. Dashboard totals label only the `active` + `unclassified` union as `operational`.

## ai-facing graph queries

`query_graph.py` provides deterministic, dependency-free JSON responses without loading the full graph into an
agent context. It auto-loads sibling `coverage.json`, `completeness.json`, `component-graph.json`, and
`query-packs.json` files when present:

```sh
GRAPH=target/runtime-interaction-graph/interaction-graph.json
python3 scripts/runtime-interaction-graph/query_graph.py --graph "$GRAPH" summary
python3 scripts/runtime-interaction-graph/query_graph.py --graph "$GRAPH" search pallet:omnipool --scope all
python3 scripts/runtime-interaction-graph/query_graph.py --graph "$GRAPH" node pallet:omnipool
python3 scripts/runtime-interaction-graph/query_graph.py --graph "$GRAPH" neighbors pallet:omnipool --depth 2
python3 scripts/runtime-interaction-graph/query_graph.py --graph "$GRAPH" paths pallet:route-executor boundary:evm-execution --view components --max-depth 5
python3 scripts/runtime-interaction-graph/query_graph.py --graph "$GRAPH" packs --section component_cycles
```

Search ranks an exact node ID first, followed by ID prefixes/substrings, exact metadata, matching edge endpoints,
and general JSON evidence. Neighbor retrieval is a bounded ego subgraph (`--depth` is capped at 6); path search is
bounded by depth, path count, and expansion count. Use `paths --view components` for collapsed runtime-component
reachability and the default `raw` view for exact node/edge evidence. Raw neighbors and searches keep parallel
evidence edges separate. Component paths group parallel variants per hop and retain their count, fingerprints,
edge kinds, traversals, representative edge, and bounded variant records, producing one record per unique node path.

Every response uses the stable envelope documented by `query-response.schema.json`. It includes the input graph's
SHA-256, SHA-256 identities for every loaded companion, query parameters, matched/returned counts, deterministic
truncation reasons, companion-coverage warnings, and an approximate token budget. `result.total_is_exact` states
whether `matched` is an exact total; bounded traversal results use `false` and `omitted: null` when the search stops
before the total can be known. `--max-records` and `--max-tokens` are hard output bounds. The main and component
graphs must use schema version 2, and the component graph must be the execution projection. Missing
`runtime_active` metadata is reported as `unclassified`: default queries exclude explicit `false` values and nodes
that inherit `false` from their owner or source function, while retaining unclassified records. Pass
`--include-inactive` to search or traverse raw inventory.

For multiple agent queries, `batch` reads JSON objects from stdin and emits one compact response per line. Per-query
budgets may reduce, but cannot exceed, the process-level limits:

```sh
printf '%s\n' \
  '{"operation":"search","text":"pallet:omnipool","max_records":10}' \
  '{"operation":"neighbors","id":"pallet:omnipool","depth":2,"max_tokens":2000}' |
  python3 scripts/runtime-interaction-graph/query_graph.py \
    --graph "$GRAPH" --max-records 50 --max-tokens 4000 batch
```

## containerized dashboard and api

Build one image containing the static dashboard and `/api/v1/query`. Generated files are supplied through a
BuildKit named context because `target/` remains excluded from the repository build context:

```sh
GRAPH=target/runtime-interaction-graph/complete-ai-interface-v3
BUILD_DATE=2026-07-16T00:00:00Z
VCS_REF=local
VERSION=local
GRAPH_SHA256=$(sha256sum "$GRAPH/interaction-graph.json" | awk '{print $1}')
RUNTIME_SPEC_VERSION=432
docker buildx build --load \
  --build-context graph="$GRAPH" \
  --build-arg BUILD_DATE --build-arg VCS_REF --build-arg VERSION \
  --build-arg GRAPH_SHA256 --build-arg RUNTIME_SPEC_VERSION \
  -f scripts/runtime-interaction-graph/Dockerfile \
  -t hydration-runtime-graph:local .
```

The default Python Alpine base is digest-pinned and can be replaced with `--build-arg PYTHON_IMAGE=...`. Large
HTML, JSON, and SVG files are deterministically gzip-compressed during the build; the uncompressed files remain
available for clients without gzip support. The runtime uses UID/GID `10001`, writes no application state, and
works with a read-only root filesystem:

```sh
docker run --rm --read-only --cap-drop=ALL --security-opt=no-new-privileges \
  -p 8080:8080 hydration-runtime-graph:local
curl --fail http://127.0.0.1:8080/healthz
curl --fail --request POST http://127.0.0.1:8080/api/v1/query \
  --header 'Content-Type: application/json' \
  --data '{"operation":"summary"}'
```

The service exposes:

- `/` — redirect to the clickable interaction explorer
- `/healthz` — container health check
- `/readyz` — loaded graph identity, counts, and companion fingerprints
- `/api/v1` — graph metadata, fingerprints, operations, and server limits
- `/api/v1/query` — deterministic JSON query endpoint (`POST`)

Published images can be retrieved with `docker pull <namespace>/hydration-runtime-graph:<tag>`. Docker Hub stores
and distributes the image; it does not run or host the dashboard/API service. Start the pulled image with
`docker run` as above, or deploy it through a container orchestrator.

RAPx call graphs can be merged as compiler-derived evidence:

```sh
export RAPX_CLEAN=false
export CXXFLAGS="-include cstdint"
cargo +nightly-2025-12-06 rapx --timeout 180 analyze callgraph -- -p pallet-hsm --lib \
  > target/runtime-interaction-graph/rapx-hsm.txt 2>&1
python3 scripts/runtime-interaction-graph/runtime_interaction_graph.py \
  --rapx-output target/runtime-interaction-graph/rapx-hsm.txt --rapx-owner pallet:hsm
```

RAPx requires its pinned nightly and should run without `-D warnings`, because newer-nightly
style lints are unrelated to semantic analysis. The imported graph keeps `semantic_source:
rapx` provenance on every accepted edge.

Collect every custom pallet, precompile, and runtime package into a normalized manifest:

```sh
python3 scripts/runtime-interaction-graph/collect_rapx.py --analysis callgraph
python3 scripts/runtime-interaction-graph/runtime_interaction_graph.py \
  --rapx-manifest target/runtime-interaction-graph/rapx/manifest.json
```

Additional `--analysis mir --analysis dataflow` flags request deeper compiler evidence.
The collector hashes the runtime source tree, collector, command inputs, and every artifact. It validates
the expected output marker and records `invalid-output` when RAPx
replays callgraph output for another analysis. Package status and artifact paths are written
incrementally, so a failed, invalid, or timed-out crate does not hide successful results.

RAPx currently may replay cached callgraph output for MIR and dataflow requests. Such output
is marked `invalid-output` and is never treated as MIR evidence. Collect genuine rustc MIR for
the high-risk routing and AMM components instead:

```sh
python3 scripts/runtime-interaction-graph/collect_mir.py
python3 scripts/runtime-interaction-graph/runtime_interaction_graph.py \
  --rapx-manifest target/runtime-interaction-graph/rapx/manifest.json \
  --rustc-mir-manifest target/runtime-interaction-graph/mir/manifest.json
```

The default MIR set is Router, HSM, Omnipool, and Stableswap. Repeat `--package` to select a
different set, or pass `--all` to collect every runtime pallet and precompile package. Successful
manifest entries are reused only when source, collector, command, and artifact hashes still match;
`--force` rebuilds selected packages. MIR instances retain monomorphized symbol identity, ordinary
calls, and separate normal/unwind successors. Basic-block control flow establishes whether a storage write can execute
before or after a conservative external call; lexical source ordering remains a separate
triage signal.

Workspace collection records package failures without discarding successful MIR. Local `std` feature
propagation is kept complete so all runtime packages currently produce MIR on the pinned nightly, and
the optional zero-failure CI budget is enforced whenever a MIR manifest is supplied. Unwind successors
are retained as conservative control-flow evidence; a reachable unwind edge does not by itself prove rollback.

The graph also models inbound/outbound XCM execution, Polkadot SDK/ORML/Frontier runtime dependencies,
the configured asset transactor, and every static or dynamic precompile route as
separate components. Function nodes receive first-class entrypoint nodes for pallet extrinsics,
runtime hooks, runtime-resolved callbacks, precompile selectors and dispatchers, EVM adapters,
and inbound XCM execution. Runtime-pallet instances retain their alias, instance parameter, index,
and excluded parts.

`semantic-inventory.json` adds source-hashed token and liquidity semantics that syntax alone cannot
name reliably: Balances/ORML/ERC-20 currency routing, StableSwap reserves and issued shares, XYK
`TotalLiquidity` and routed share issuance, Omnipool positions/protocol shares/hub backing, and the
issuance-increase circuit breaker. Enforcement labels distinguish runtime checks, transactional checks,
coupled updates, calculations, configuration, observation, and try-runtime validation.

Deployment artifacts from the sibling Aave, Hollar, and WHM repositories can be normalized and
merged into the graph:

```sh
python3 scripts/runtime-interaction-graph/collect_contracts.py \
  --descriptor scripts/runtime-interaction-graph/deployment-sources.json \
  --output target/runtime-interaction-graph/contracts.json
python3 scripts/runtime-interaction-graph/runtime_interaction_graph.py \
  --contracts-manifest target/runtime-interaction-graph/contracts.json
```

`deployment-sources.json` is the reviewed schema-v2 input inventory. It explicitly selects the mainnet-relevant
`hydration` and `gigahdx` Aave/Hollar directories and the production WHM descriptors. Missing sources,
empty selections, and malformed artifacts fail collection. Every record retains its artifact SHA-256;
the manifest records the descriptor hash, aggregate input hashes, sibling repository commits and dirty
state, and collector version. Each chain pins its EVM chain ID, Substrate genesis hash, and runtime spec name.
Nested tuple ABI inputs are recursively canonicalized and emitted with their canonical four-byte EVM selectors.
WHM deployment-step outputs are split into explicit implementation/proxy/contract aliases and typed address
references such as assets, oracles, handlers, bridge endpoints, and authorization accounts. References are
retained as deployment-step relationships but are not submitted to RPC enrichment or classified as deployed code.

Pin and enrich deployment records with live EVM bytecode, EIP-1967 implementations, and runtime
storage configuration:

```sh
python3 scripts/runtime-interaction-graph/enrich_contracts_rpc.py \
  --input target/runtime-interaction-graph/contracts.json \
  --output target/runtime-interaction-graph/contracts-onchain.json \
  --rpc https://hdx.tarn.hydration.cloud \
  --block 0xc8d433 \
  --expected-chain-id 222222 \
  --network hydration \
  --network gigahdx
node scripts/runtime-interaction-graph/collect_runtime_contracts.mjs \
  --input target/runtime-interaction-graph/contracts-onchain.json \
  --output target/runtime-interaction-graph/contracts-runtime.json \
  --rpc wss://rpc.hydradx.cloud \
  --block-number 13161523 \
  --expected-genesis-hash 0xafdc188f45c71dacbaa0b62e16a91f726c7b8699a9748cdf715459de6b7f366d \
  --expected-spec-name hydradx
python3 scripts/runtime-interaction-graph/runtime_interaction_graph.py \
  --contracts-manifest target/runtime-interaction-graph/contracts-runtime.json
```

All paths, RPCs, chains, networks, and blocks are explicit. The EVM collector verifies the RPC chain ID
and records the EVM block, parent, and state-root identity. The runtime collector rejects legacy input,
requires declared collection/enrichment provenance, and enforces the same block number, expected Substrate
genesis hash, and expected runtime
spec name, and records the Substrate genesis/block/header and runtime-version identity plus its input and tool
hashes. All five Hydration storage query APIs must be present, per-query calls and record counts are retained,
and the chain descriptor enforces minimum GigaHDX, HSM, Liquidation, total, and Asset Registry ERC-20
configuration counts. WHM Base,
Ethereum, and other external deployments
require separate descriptors and runs against their own RPCs. EVM observations include code presence,
SHA-256, code size, EIP-1967 implementation, canonical `eip155:<chain-id>:<address>` identity, and embedded
known addresses. Runtime storage covers GigaHDX, HSM, Liquidation, and Asset Registry assets whose type is
`Erc20` and whose `AssetLocations` value is exactly an `X1(AccountKey20)` location.
Compare two pinned snapshots with
`diff_snapshots.py before.json after.json`; changes include code, implementation, and embedded
contract relationships. The focused `deployed-contracts.svg` view connects runtime components,
EVM adapters, contracts, implementations, deployment steps, and typed external-address references.

Run the complete Hydration EVM pipeline with:

```sh
python3 scripts/runtime-interaction-graph/build_evm_graph.py \
  --descriptor scripts/runtime-interaction-graph/deployment-sources.json \
  --chain hydration-mainnet \
  --evm-rpc https://hdx.tarn.hydration.cloud \
  --substrate-rpc wss://rpc.hydradx.cloud \
  --block 0xc8d433 \
  --output target/runtime-interaction-graph/evm-hydration-pinned
```

It normalizes deployment artifacts, selects an EVM block, reads runtime configuration at the same
block number, enriches proxy and bytecode relationships, and renders the graph. `--block` accepts only
a pinned decimal or `0x`-prefixed block number. The output path must be new or empty so stale optional
artifacts cannot be attributed to the current build. RAPx and rustc-MIR manifests are never discovered from
the output directory; pass `--rapx-manifest` or `--rustc-mir-manifest` explicitly to include a validated,
hashed semantic input whose source fingerprint matches the current runtime tree. Rust selector enums
are connected to matching deployed-contract ABI functions. `diff_graphs.py` compares any two
`interaction-graph.json` files. Pull requests run the source graph for the base and head revisions
and upload both graphs plus `graph-diff.json` as a CI artifact.
The composite `evm-build-provenance.json` hashes every transitive graph-generator input, the active coverage
policy, source tree, compiler manifests and artifacts, chain identity, deployment inputs, and every generated
output except the provenance file itself. Supplying a rustc-MIR manifest selects the full zero-failure MIR
coverage policy.

Integration-test comparison is conservative: direct source references link tests to graph components and
entrypoints, while interaction linkage requires both edge endpoints to occur in the same test. Tests are
split into reference, assertion, and direct-dispatch-assertion confidence tiers. None of these static tiers
equates source mention or co-occurrence with observed behavioral coverage.

`coverage-thresholds.json` protects minimum graph, operational projection, and operational integration-test linkage
counts, requires zero unresolved runtime targets, and pins the reviewed inventory-only target IDs and reason
counts exactly. Raw-inventory counts remain available as compatibility and completeness metrics. Pull-request graphs add
`fixtures/ci-contracts.json` and apply
`coverage-thresholds-deployment.json`, which requires deployed contracts, deployment edges, and runtime EVM
configuration bindings without accessing RPC endpoints or sibling deployment repositories.
`coverage-thresholds-full.json` additionally requires all 46 requested MIR packages to succeed and enforces
aggregate matched-function, monomorphized-instance, and imported-operation floors so a nonempty but
unparsable MIR corpus cannot pass. Pull-request diffs apply the trusted base policy and explicit per-field
regression budgets.

## Security model

Execution domains are represented separately: FRAME pallets, runtime adapters, EVM,
precompiles, and runtime configuration bindings. Currency calls remain polymorphic so
future enrichment can branch them into Balances, ORML Tokens, real ERC-20, mapped ERC-20,
XCM, pool-share, and protocol-minted asset behavior.
Asset-kind edges are grounded in `pallet_asset_registry::AssetType`. `Token` remains
polymorphic because the runtime adapter may route it through native Balances, ORML
Tokens, a protocol-controlled implementation, or the mapped ERC-20 facade.

State/external-call candidates use storage writes, not reads. The report records whether
writes occur before or after the first conservative external boundary. This ordering is a
triage signal; Rust source order alone does not establish reentrancy or rollback behavior.

Run the extractor regression tests with:

```sh
python3 -m unittest discover -s scripts/runtime-interaction-graph -p 'test_*.py'
node --test scripts/runtime-interaction-graph/collect_runtime_contracts.test.mjs
```
