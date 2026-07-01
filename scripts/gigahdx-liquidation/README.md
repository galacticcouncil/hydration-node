# gigahdx-liquidation

End-to-end test for `pallet-liquidation::liquidate_gigahdx` against a local
zombienet fork of lark2. Verifies the routing fix in
`pallets/liquidation/src/lib.rs` and the `approve_contract(GIGAHDX_POOL)`
governance precondition (see `aave-v3-deploy/.claude/GIGAHDX-LARK-DEPLOY-RUNBOOK.md`
Phase 7.6).

## Prerequisites

1. Chain running on `ws://127.0.0.1:9999` (override with `WS_URL`).
2. Chain state must include the GIGAHDX pool deployed and Phase 7 enacted.
   The standard local-fork harness in `launch-configs/fork/` produces this.
3. `//Alice` must be the sole TC member (default on local zombienet).
4. Node 18+.

## Run

```sh
npm install
npm test
```

## What the test does

| # | Step | Why |
|---|------|-----|
| 1 | `ensureGigahdxPoolApproved` | Phase 7.6 precondition. Without it AAVE's HOLLAR transferFrom underflows and every liquidation panics with `Panic(0x11)`. |
| 2 | `ensureLiquidator` | //Bob acts as the gigahdx liquidation account; needs collateral on the MAIN pool so the HOLLAR flash-borrow during liquidation succeeds. |
| 3 | `setupBorrower` | Creates a fresh //LIQTEST_BORROWER position (100K HDX stake → ~$220 collateral, 80 HOLLAR borrow). |
| 4 | `dropStHdxPrice` | Crashes stHDX from $0.025 to $0.01 to push HF<1. |
| 5 | `liquidate(asset=stHDX, ...)` | Asserts `GigaHdxLiquidated` event — proves PEPL's production path works. |
| 6 | `liquidate(asset=GIGAHDX, ...)` | Asserts the same event — proves the OR-clause accepts both asset ids. |

## Layout

```
scripts/gigahdx-liquidation/
├── package.json
├── tsconfig.json
├── .mocharc.json
└── src/
    ├── api.ts          # ApiPromise + Alice/Bob signers
    ├── constants.ts    # lark2 addresses + asset ids
    ├── utils.ts        # signAndWait, hex helpers
    ├── governance.ts   # TC-direct approve_contract
    ├── liquidator.ts   # Bob setup (MAIN-pool collateral)
    ├── borrower.ts     # fresh borrower + stake + borrow
    ├── oracle.ts       # FixedPriceOracle setPrice (deployer key)
    └── liquidation.ts  # pallet_liquidation::liquidate wrapper
└── test/
    └── e2e.test.ts     # mocha + chai
```

## Caveats

- The oracle helper uses `FixedPriceOracle.setPrice` via the documented public
  dev deployer key. That key is hard-coded in lark deployments and is NOT a
  secret — it's published in `aave-v3-deploy/deployments/lark2/_addresses.md`.
  Do not reuse it on mainnet.
- `ensureGigahdxPoolApproved` goes through `TC.propose(threshold=1)` because
  Alice is the sole TC member on local zombienet. On lark/mainnet the same
  call goes through `WhitelistedCaller` or `GeneralAdmin` referenda — see
  `aave-v3-deploy/scripts/lark/approve-gigahdx-as-controller.ts`.
- The test borrower is funded by //Bob, so //Bob must have spare HDX on the
  snapshot. Default lark2 snapshot satisfies this.
