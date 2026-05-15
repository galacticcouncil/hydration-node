# ICE security-probe scripts

Standalone JavaScript probes used during the ICE / DcaIntent security
review. Companion to
`../../integration-tests/src/ice/SECURITY_FINDINGS.md` and
`../../integration-tests/src/ice/ATTACK_IDEATION.md`. Not part of the
cargo build.

Targets a real zombienet (the `lark*` test networks). No chopsticks
support.

## Files

| File | Purpose |
|---|---|
| `ddos.mjs` | Intent-bloat DDoS probe — derives N fresh accounts, funds them, submits an unresolvable intent each. Targets storage growth + solver-starvation hypothesis (`SECURITY_FINDINGS.md §B5`). |

## Setup

```bash
cd scripts/ice-security-probe
npm install
```

⚠️ **Important — install dependencies LOCALLY**: do not rely on a
`node_modules/` higher up the directory tree. Some dev environments have
a stale `@polkadot/api` (v10.x) at `~/dev/node_modules` that Node will
resolve in preference; the older client encodes extrinsics in a way
that modern Hydration runtimes panic on during `validate_transaction`
(`wasm trap: unreachable`). The local `package.json` here pins
`@polkadot/api ^15.9.1`.

## Run `ddos.mjs`

Edit the **top-of-file constants** before running:

| Constant | Default | Notes |
|---|---|---|
| `ENDPOINT` | `wss://2.lark.hydration.cloud` | Hardcoded on purpose. Change in source — no env override. |
| `FUND_HDX` | `50_000n` | HDX per bloat account. Total funder spend = `N × FUND_HDX`. |
| `FUNDER_URI` | mnemonic | Funder URI (mnemonic / hex seed / dev derivation like `//Alice`). |
| `FUNDER_EXPECTED_ADDRESS` | `7Kg…` | Sanity-check the URI derives to the expected address; abort otherwise. |

Env knobs (safe — do **not** set ENDPOINT here):

```bash
N=50 INTENTS_PER_ACCOUNT=1 node ddos.mjs
N=200 INTENTS_PER_ACCOUNT=100 node ddos.mjs    # 20 000 intents = max bloat (per-account cap is 100)
```

## What it does

1. **Phase 0** — connect, log chain name + head.
2. **Phase 1** — derive `N` deterministic sr25519 keypairs from
   `${ROOT_SEED}//ddos-probe//${i}`.
3. **Phase 2** — fund each via `transferKeepAlive` from `FUNDER_URI`.
   Refuses to run unless the derived funder address matches
   `FUNDER_EXPECTED_ADDRESS` and the funder has ≥ `N × FUND_HDX` free.
   Sequential — submits one transfer at a time and polls
   `system.account.nonce` for inclusion before sending the next.
4. **Phase 3** — submit one (or `INTENTS_PER_ACCOUNT`) intent per
   account:

   ```
   Swap {
     asset_in:  HDX (0),
     asset_out: BNC (14),
     amount_in: 1 HDX (= ED, smallest valid reserve),
     amount_out: 10^30 BNC (unreachably high — solver never fills),
     partial:    false,
   }
   deadline:   now + 23 h (just under MaxAllowedIntentDuration of 24 h)
   ```

   Shape empirically verified on lark to **stay unresolved** for 30+
   blocks of observation (no `IntentResolved` event fires).

5. **Phase 4** — verify:
   - `intent.Intents::entries().length` (global count after the bloat)
   - sum of `accountIntentCount` over the bloat accounts
   - head-block delta during the run

   Final summary printed as JSON.

## Cleanup

This script does **not** auto-cancel its intents — they sit until
either their 23 h deadline elapses (then the OCW's `cleanup_intent`
will reap up to 10 per block) or a separate cancel pass is run.

## Safety notes

- `ENDPOINT` is a const, not env-overridable, to avoid accidentally
  pointing the script at the wrong network.
- `FUNDER_EXPECTED_ADDRESS` guards against the URI deriving to an
  unexpected account on a chain with a different ss58 prefix.
- The unresolvable shape uses `amount_in = 1 HDX` (existential deposit)
  precisely to keep the per-intent reserve small. Total reserved across
  N intents = `N × 1 HDX`.
