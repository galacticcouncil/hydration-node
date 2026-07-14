# signet-feeless-test

Chopsticks e2e for the allowlist-gated `signet.respond` flow: the fee is locked
upfront and refunded when the call succeeds, otherwise charged. An authorized
signer's successful `respond` costs nothing (fee refunded), while a
non-allowlisted account fails and is charged. The caller must hold enough HDX to
lock the fee.

Mirrors the pallet unit tests in `pallets/signet/src/tests/signer_allowlist.rs`.

## Run

Build the runtime wasm and install deps:

```sh
cargo build --release -p hydradx-runtime
cd scripts/signet-feeless-test && yarn install
```

Then run (launches a Chopsticks fork of hydradx, runs the tests, tears down):

```sh
./run.sh
```

`add_signer` is dispatched as Root via the scheduler on the fork; on a live
chain it goes through governance (`UpdateOrigin = EnsureRoot | TechCommittee`).
