# OpenGov Force Remove Votes

Builds `convictionVoting.forceRemoveVote(target, class, refIndex)` calls for
finished referenda found in `ConvictionVoting::VotingFor`.

The signer must satisfy the runtime `VoteRemovalOrigin` configured for
`pallet-conviction-voting` (on Hydration, a Technical Committee member after the
SDK patch that enables this call).

## Usage

```sh
cd scripts/opengov-force-remove-votes
npm install

RPC_SERVER=wss://rpc.hydradx.cloud npm run dry-run

RPC_SERVER=wss://rpc.hydradx.cloud npm run submit
```

Submit mode prompts for `ACCOUNT_SECRET` interactively, so the seed is not
written to shell history.

If the script reports unsupported signed extensions, refresh dependencies:

```sh
rm -rf node_modules package-lock.json
npm install
```

The script logs `ConvictionVoting::VotingFor` record and vote counts before
building calls, and again after submitted batches complete.

Optional environment variables:

- `RPC_SERVER` - chain RPC, defaults to `ws://127.0.0.1:9944`
- `BATCH_SIZE` - calls per `utility.batch`, defaults to `20`
- `LIMIT` - maximum force-remove calls to build, useful for staged runs
- `TX_TIMEOUT_MS` - per-batch inclusion timeout before skipping, defaults to `60000`

Batches that do not reach inclusion before `TX_TIMEOUT_MS` are skipped. Re-run
the script later to rescan chain state and pick up any skipped votes.
