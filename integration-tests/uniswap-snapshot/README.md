# uniswap-snapshot

Snapshot-backed integration tests for the Uniswap v3 router venue
(`src/uniswap_v3_router.rs`). The tests load a scraped EVM state snapshot that
already contains a deployed Uniswap v3 stack (Factory, SwapRouter02, QuoterV2,
a pool with liquidity), then drive the runtime router against it — the same
pattern as `aave_router.rs` / `evm-snapshot/`.

The tests are `#[ignore]`d until the `SNAPSHOT` artifact and the deployment
constants are committed (the binary is too large to ship by default, and the
addresses depend on the deployment used). Follow the steps below to produce them.

## 1. Bring up a chain with Uniswap v3 deployed

Use the local zombienet + Uniswap v3 deploy from the `ys-` branches (the
`uniswap-v3-deploy` sibling repo). The chain must run **this** runtime
(`feat/uniswap-v3-router`) so the snapshot is loadable, and must have:

- Factory, SwapRouter02, QuoterV2 deployed.
- At least one pool created **and initialized** for an asset pair that maps to
  two registered Hydration assets, with liquidity minted (via the
  NonfungiblePositionManager) so quotes and swaps return non-zero.
- `ALICE` funded with `ASSET_IN`.

Record the deployed addresses (the deploy writes them to
`uniswap-v3-deploy/deployments/<network>/_addresses.json`).

## 2. Build the scraper

```bash
cargo build --release -p scraper
```

## 3. Scrape the EVM + registry + balances into a snapshot

```bash
./target/release/scraper save-storage \
    --pallet EVM AssetRegistry System Tokens Omnipool Timestamp Parameters \
    --uri ws://127.0.0.1:9988 \
    --path integration-tests/uniswap-snapshot
```

This writes `integration-tests/uniswap-snapshot/SNAPSHOT_<block>`. Rename it to
`SNAPSHOT` (or update `PATH_TO_SNAPSHOT` in `src/uniswap_v3_router.rs`). Add
`--slim` to drop most user accounts and keep the file small.

> Including the `Parameters` pallet means that if you call
> `parameters.setUniswapV3Addresses` on the source chain **before** scraping,
> the addresses are baked into the snapshot and `with_uniswap_v3` doesn't need
> to set them — in that case the `UNISWAP_V3_*` constants below are only used as
> a fallback.

## 4. Fill in the deployment constants

In `src/uniswap_v3_router.rs` set, from the deploy's `_addresses.json` and your
asset registry:

- `UNISWAP_V3_FACTORY`, `UNISWAP_V3_SWAP_ROUTER`, `UNISWAP_V3_QUOTER`
- `ASSET_IN`, `ASSET_OUT`, `FEE_TIER` — the pair + fee tier of the seeded pool
- `SELL_AMOUNT` — comfortably below the pool's depth

## 5. Run the tests

```bash
cargo test -p runtime-integration-tests uniswap_v3_router -- --ignored
```

Drop the `#[ignore]` attributes once `SNAPSHOT` and the constants are committed.
