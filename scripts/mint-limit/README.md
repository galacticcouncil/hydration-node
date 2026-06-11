# mint-limit

Scripts that generate Technical Committee proposal hex for managing per-asset XCM mint limits and lifting circuit breaker lockdowns.

Submit the printed `HEX` via polkadot.js → `technicalCommittee.propose` (as a call/hex).

## Setup

```bash
cd scripts/mint-limit
npm install
```

## `createProposal.js` — set mint limits for a list of assets

Generates a TC proposal that updates `xcmRateLimit` for each asset in the `ASSETS` array.

1. Edit `ASSETS` at the top of the script to the asset IDs you want covered.
2. (Optional) Override the computed limit for specific assets via `MINT_LIMIT_OVERWRITES`.
3. Run:

   ```bash
   node createProposal.js
   ```

4. Copy the printed TC propose HEX and submit it via polkadot.js.

The script queries Grafana for trade volume and computes limits accordingly; the only knob most runs need is the `ASSETS` list.

## `liftLockdown.js` — lift a circuit breaker lockdown for one asset

Generates a TC proposal that calls `circuitBreaker.forceLiftLockdown(ASSET_ID)`.

1. Edit `ASSET_ID` at the top of the script.
2. Run:

   ```bash
   node liftLockdown.js
   ```

3. Copy the printed TC propose HEX and submit it via polkadot.js.

If you also need to raise `xcmRateLimit` (so the same deposit pattern doesn't immediately re-lock the asset), use `ai_skills/circuit-breaker-incident/scripts/generate-tc-unlock.js` instead — it batches the lift + the limit raise into a single proposal.
