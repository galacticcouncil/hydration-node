# Democracy vote cleanup

Pre-cleans Gov v1 before the runtime upgrade that removes `pallet-democracy`.
Adapted from `../opengov-force-remove-votes`.

The script reads a finalized snapshot, selects direct votes on finished or
cancelled referenda, and builds `democracy.forceRemoveVote(target, index)` calls.
It then adds `democracy.unlock(target)` for selected voters and direct voters
with existing democracy locks, including accounts whose votes were removed in
an earlier run. Existing staking hooks settle earned legacy points.

Run submit mode with a **technical committee member's signed account**. A
collective committee proposal has the wrong origin. The secret is requested
interactively with terminal echo suppressed.

```sh
cd scripts/democracy-force-remove-votes
npm ci
npm test

RPC_SERVER=wss://rpc.hydradx.cloud npm run dry-run
RPC_SERVER=wss://rpc.hydradx.cloud LIMIT=20 npm run submit
RPC_SERVER=wss://rpc.hydradx.cloud npm run submit
```

Dry-run performs reads and writes `cleanup-plan.json` containing the snapshot,
selected calls, and encoded `utility.batchAll` call data for every batch. It
does not sign, simulate dispatch, or submit transactions. Submit mode creates
a fresh plan, checks committee membership and batch weight, then sends batches
sequentially and waits for finalization. Failed atomic batches or uncertain
outcomes stop the run. Check finalized state and rerun to plan remaining work.

Each batch contains at most `BATCH_SIZE` total calls, including unlocks. All
vote removals precede unlocks; a timeout or failure can leave unlocks for the
next run. Other cleanup transactions running concurrently can invalidate a
plan; rerun after they finish.

Configuration:

| Variable | Default | Meaning |
| --- | --- | --- |
| `RPC_SERVER` | `ws://127.0.0.1:9944` | Chain WebSocket endpoint |
| `BATCH_SIZE` | `20` | Maximum calls per atomic batch; lower if overweight |
| `LIMIT` | unset | Maximum votes selected; unlock calls are additional |
| `TX_TIMEOUT_MS` | `180000` | Per-batch finalization timeout |
| `OUTPUT_FILE` | `cleanup-plan.json` | File for the generated plan |

Before and after submission, the script reports democracy key count, vote and
delegation counts, democracy locks on recorded voters, proposal deposits, and
legacy staking record and vote counts. A dry-run after cleanup provides a
fresh inventory for sizing the removal migration.

Ongoing votes and delegations are skipped. Force removal can move an unexpired
conviction obligation into a prior lock; `unlock` releases only expired locks.
Empty voting and staking records remain, proposal deposits are not refunded by
this script, and orphan staking records can remain. A removal migration is
still required. Choose a single-block migration only after measuring the
remaining storage, work, and proof size. Run this script while democracy and
the legacy staking hooks are still present on chain.
