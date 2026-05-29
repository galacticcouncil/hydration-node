# dca-monitor

Drives a Chopsticks fork forward by N blocks and prints a per-schedule timeline of DCA events. Use this to verify a DCA fix on a forked chain *before* deploying the runtime upgrade.

## Typical flow: verify a DCA fix on a fork

1. Fork prod with Chopsticks at (or shortly before) the problematic block.
2. Apply your runtime upgrade to the fork.
3. Run `dca-monitor` against the fork — it forces 150 blocks of progress and surfaces every relevant event grouped by schedule ID.
4. Confirm the previously-failing schedule now executes as expected.

## Setup

```bash
cd scripts/dca-monitor
npm install
```

Chopsticks fork must be reachable at `ws://localhost:8000` (the default endpoint hardcoded in `index.js`; edit `ENDPOINT` to change). `BLOCK_COUNT` controls how many blocks the script drives (default 150).

## Run

```bash
node index.js
```

The script signs `system.remark` extrinsics with `//Alice` to force block production, then reads `system.events` at each new block.

## What it tracks

Grouped per-schedule:
- Schedule executions and outcomes.
- DCA failures and the decoded error.
- Randomness failures.
- Reserve unlocks.

Plus global trackers:
- Extrinsic failures.
- Router executions and swap events.
- Dust loss events.
